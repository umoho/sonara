//! Firewheel 后端适配层

use std::{
    collections::{HashMap, HashSet},
    num::{NonZeroU32, NonZeroUsize},
    sync::mpsc,
    thread,
};

use firewheel::{
    FirewheelConfig, FirewheelContext,
    clock::{DurationSeconds, EventInstant},
    collector::ArcGc,
    cpal::CpalConfig,
    nodes::sampler::{PlayFrom, RepeatMode, SamplerNode, SamplerState},
    sample_resource::{InterleavedResourceF32, SampleResource},
};
use firewheel_pool::{NewWorkerError, SamplerPoolVolumePan, WorkerID};
use firewheel_symphonium::{DecodedAudio, load_audio_file};
use sonara_build::{BuildError, CompiledBankPackage};
use sonara_model::{
    AudioAsset, Bank, BankAsset, BankId, BankManifest, Bus, Clip, ClipId, Event, EventId,
    MusicGraph, MusicGraphId, MusicStateId, ParameterId, ParameterValue, ResumeSlot, Snapshot,
    SyncDomain, TrackId,
};
use sonara_runtime::{
    AudioCommandOutcome, EmitterId, EventInstanceId, EventInstanceState, Fade, MusicPhase,
    MusicSessionId, MusicStatus, PlaybackPlan, ResolvedMusicPlayback, RuntimeCommandBuffer,
    RuntimeError, RuntimeRequest, RuntimeRequestResult, SonaraRuntime,
};
use thiserror::Error;
use uuid::Uuid;

/// Firewheel 后端错误
#[derive(Debug, Error)]
pub enum FirewheelBackendError {
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("资源 `{0}` 没有注册到 Firewheel 后端")]
    AssetNotRegistered(Uuid),
    #[error("资源 `{0}` 的声道数必须大于 0")]
    InvalidChannelCount(Uuid),
    #[error("资源 `{0}` 的采样率必须大于 0")]
    InvalidSampleRate(Uuid),
    #[error("资源 `{0}` 解码失败: {1}")]
    DecodeAsset(Uuid, String),
    #[error("Firewheel 启动音频流失败: {0}")]
    StartStream(String),
    #[error("Firewheel 更新失败: {0}")]
    Update(String),
    #[error("Firewheel worker 创建失败: {0}")]
    NewWorker(#[from] NewWorkerError),
    #[error("播放位置 `{0}` 必须是非负有限秒数")]
    InvalidPlaybackPosition(f64),
    #[error("调度延迟 `{0}` 必须是非负有限秒数")]
    InvalidScheduleDelay(f64),
    #[error("clip `{0:?}` is not loaded")]
    ClipNotLoaded(ClipId),
    #[error("clip `{0:?}` 的时间范围非法")]
    InvalidClipRange(ClipId),
    #[error("clip `{0:?}` 的子区间循环暂未接入 Firewheel sampler")]
    UnsupportedClipLoopRange(ClipId),
}

/// Firewheel backend 可消费的一条最小请求
pub type FirewheelRequest = RuntimeRequest;

/// Firewheel backend 执行请求后的结果
pub type FirewheelRequestResult = RuntimeRequestResult;

/// Firewheel backend 在隔离模式下处理单条请求后的结果
pub type FirewheelRequestOutcome =
    AudioCommandOutcome<FirewheelRequest, FirewheelRequestResult, FirewheelBackendError>;

const MUSIC_SCHEDULE_EARLY_SECONDS: f64 = 0.020;

/// 真实后端返回的一个实例播放头快照。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstancePlayhead {
    pub position_seconds: f64,
    pub worker_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingMusicPlayback {
    clip_id: ClipId,
    track_id: Option<TrackId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingExitCue {
    target_position_seconds: f64,
    target_audio_time_seconds: Option<f64>,
    waiting_for_wrap: bool,
    last_position_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingBridgeCompletion {
    target_audio_time_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedClipPlayback {
    asset_id: Uuid,
    start_from_seconds: f64,
    stop_after_seconds: Option<f64>,
    repeat_mode: RepeatMode,
}

/// 基于 Firewheel 的最小 one-shot 播放后端
pub struct FirewheelBackend {
    runtime: SonaraRuntime,
    context: FirewheelContext,
    sampler_pool: SamplerPoolVolumePan,
    known_bank_assets: HashMap<Uuid, BankAsset>,
    loading_streaming_assets: HashSet<Uuid>,
    pending_playbacks: HashMap<EventInstanceId, PlaybackPlan>,
    pending_music_playbacks: HashMap<MusicSessionId, PendingMusicPlayback>,
    pending_exit_cues: HashMap<MusicSessionId, PendingExitCue>,
    pending_bridge_completions: HashMap<MusicSessionId, PendingBridgeCompletion>,
    sample_resources: HashMap<Uuid, ArcGc<dyn SampleResource>>,
    streaming_asset_tx: mpsc::Sender<StreamingAssetLoadResult>,
    streaming_asset_rx: mpsc::Receiver<StreamingAssetLoadResult>,
    instance_workers: HashMap<EventInstanceId, Vec<WorkerID>>,
    worker_instances: HashMap<WorkerID, EventInstanceId>,
    music_session_workers: HashMap<MusicSessionId, Vec<WorkerID>>,
    worker_music_sessions: HashMap<WorkerID, MusicSessionId>,
    music_session_stinger_workers: HashMap<MusicSessionId, Vec<WorkerID>>,
    worker_music_stingers: HashMap<WorkerID, MusicSessionId>,
    active_music_clips: HashMap<MusicSessionId, ClipId>,
    active_music_tracks: HashMap<MusicSessionId, Option<TrackId>>,
    command_buffer: RuntimeCommandBuffer,
}

impl FirewheelBackend {
    /// 使用现有运行时创建后端, 并立即启动默认输出流
    pub fn new(runtime: SonaraRuntime) -> Result<Self, FirewheelBackendError> {
        let mut context = FirewheelContext::new(FirewheelConfig::default());
        context
            .start_stream(CpalConfig::default())
            .map_err(|error| FirewheelBackendError::StartStream(error.to_string()))?;

        let sampler_pool = SamplerPoolVolumePan::new(
            32,
            SamplerNode::default(),
            None,
            context.graph_out_node_id(),
            firewheel::channel_config::NonZeroChannelCount::STEREO,
            &mut context,
        );

        context
            .update()
            .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        let (streaming_asset_tx, streaming_asset_rx) = mpsc::channel();

        Ok(Self {
            runtime,
            context,
            sampler_pool,
            known_bank_assets: HashMap::new(),
            loading_streaming_assets: HashSet::new(),
            pending_playbacks: HashMap::new(),
            pending_music_playbacks: HashMap::new(),
            pending_exit_cues: HashMap::new(),
            pending_bridge_completions: HashMap::new(),
            sample_resources: HashMap::new(),
            streaming_asset_tx,
            streaming_asset_rx,
            instance_workers: HashMap::new(),
            worker_instances: HashMap::new(),
            music_session_workers: HashMap::new(),
            worker_music_sessions: HashMap::new(),
            music_session_stinger_workers: HashMap::new(),
            worker_music_stingers: HashMap::new(),
            active_music_clips: HashMap::new(),
            active_music_tracks: HashMap::new(),
            command_buffer: RuntimeCommandBuffer::new(),
        })
    }

    /// 获取后端持有的运行时引用
    pub fn runtime(&self) -> &SonaraRuntime {
        &self.runtime
    }

    /// 获取后端持有的运行时可变引用
    pub fn runtime_mut(&mut self) -> &mut SonaraRuntime {
        &mut self.runtime
    }

    /// 加载 bank, 事件和它依赖的音频资源
    pub fn load_bank(
        &mut self,
        bank: Bank,
        events: Vec<Event>,
    ) -> Result<(), FirewheelBackendError> {
        self.load_bank_with_definitions(
            bank,
            events,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    /// 加载一个 bank 以及和它配套的高层对象定义。
    pub fn load_bank_with_definitions(
        &mut self,
        bank: Bank,
        events: Vec<Event>,
        buses: Vec<Bus>,
        snapshots: Vec<Snapshot>,
        clips: Vec<Clip>,
        resume_slots: Vec<ResumeSlot>,
        sync_domains: Vec<SyncDomain>,
        music_graphs: Vec<MusicGraph>,
    ) -> Result<(), FirewheelBackendError> {
        self.register_bank_manifest(&bank.manifest)?;
        self.runtime.load_bank_with_definitions(
            bank,
            events,
            buses,
            snapshots,
            clips,
            resume_slots,
            sync_domains,
            music_graphs,
        )?;
        Ok(())
    }

    /// 直接加载一份完整的 compiled bank 载荷。
    pub fn load_compiled_bank(
        &mut self,
        package: CompiledBankPackage,
    ) -> Result<BankId, FirewheelBackendError> {
        let bank_id = package.bank.id;
        self.load_bank_with_definitions(
            package.bank,
            package.events,
            package.buses,
            package.snapshots,
            package.clips,
            package.resume_slots,
            package.sync_domains,
            package.music_graphs,
        )?;
        Ok(bank_id)
    }

    /// 注册一个 compiled bank manifest 引用到的所有资源。
    pub fn register_bank_manifest(
        &mut self,
        manifest: &BankManifest,
    ) -> Result<(), FirewheelBackendError> {
        let resident_media: HashSet<Uuid> = manifest.resident_media.iter().copied().collect();
        let streaming_media: HashSet<Uuid> = manifest.streaming_media.iter().copied().collect();

        for asset in &manifest.assets {
            self.known_bank_assets.insert(asset.id, asset.clone());

            if should_preload_bank_asset(asset, &resident_media, &streaming_media) {
                self.decode_bank_asset(asset)?;
            } else {
                self.prewarm_streaming_bank_asset(asset);
            }
        }

        Ok(())
    }

    /// 注册一个 compiled bank asset, 并立即准备可播放资源。
    ///
    /// 这条路径直接面向 bank 编译产物, 避免 backend 继续依赖 authoring 语义。
    pub fn register_bank_asset(&mut self, asset: &BankAsset) -> Result<(), FirewheelBackendError> {
        self.known_bank_assets.insert(asset.id, asset.clone());
        self.decode_bank_asset(asset)
    }

    fn decode_bank_asset(&mut self, asset: &BankAsset) -> Result<(), FirewheelBackendError> {
        let decoded = load_bank_asset_resource(asset)
            .map_err(|error| FirewheelBackendError::DecodeAsset(asset.id, error))?;
        self.register_sample_resource(asset.id, decoded.into());
        Ok(())
    }

    /// 注册一段交错布局的 `f32` PCM 数据
    pub fn register_interleaved_f32_asset(
        &mut self,
        asset_id: Uuid,
        channels: usize,
        sample_rate: u32,
        data: Vec<f32>,
    ) -> Result<(), FirewheelBackendError> {
        let channels = NonZeroUsize::new(channels)
            .ok_or(FirewheelBackendError::InvalidChannelCount(asset_id))?;
        let sample_rate = NonZeroU32::new(sample_rate)
            .ok_or(FirewheelBackendError::InvalidSampleRate(asset_id))?;
        let resource = InterleavedResourceF32 {
            data,
            channels,
            sample_rate: Some(sample_rate),
        };

        let resource: ArcGc<dyn SampleResource> = ArcGc::new_unsized(|| {
            std::sync::Arc::new(resource) as std::sync::Arc<dyn SampleResource>
        });
        self.register_sample_resource(asset_id, resource);
        Ok(())
    }

    /// 注册一个自定义 SampleResource
    pub fn register_sample_resource(
        &mut self,
        asset_id: Uuid,
        resource: ArcGc<dyn SampleResource>,
    ) {
        self.sample_resources.insert(asset_id, resource);
    }

    /// 从磁盘上的音频文件加载一个 AudioAsset
    pub fn register_audio_asset(
        &mut self,
        asset: &AudioAsset,
    ) -> Result<(), FirewheelBackendError> {
        let bank_asset = BankAsset {
            id: asset.id,
            name: asset.name.clone(),
            source_path: asset.source_path.clone(),
            import_settings: asset.import_settings.clone(),
            streaming: asset.streaming,
        };
        self.known_bank_assets
            .insert(bank_asset.id, bank_asset.clone());

        if asset.streaming == sonara_model::StreamingMode::Streaming {
            Ok(())
        } else {
            self.decode_bank_asset(&bank_asset)
        }
    }

    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, FirewheelBackendError> {
        let instance_id = self.runtime.play(event_id)?;
        let plan = self
            .runtime
            .active_plan(instance_id)
            .cloned()
            .expect("active plan should exist right after play");
        self.playback_plan(instance_id, &plan)?;
        Ok(instance_id)
    }

    /// 排队一个未绑定 emitter 的播放请求
    pub fn queue_play(&mut self, event_id: EventId) {
        self.command_buffer.queue_play(event_id);
    }

    /// 在 emitter 上播放一个事件
    pub fn play_on(
        &mut self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<EventInstanceId, FirewheelBackendError> {
        let instance_id = self.runtime.play_on(emitter_id, event_id)?;
        let plan = self
            .runtime
            .active_plan(instance_id)
            .cloned()
            .expect("active plan should exist right after play_on");
        self.playback_plan(instance_id, &plan)?;
        Ok(instance_id)
    }

    /// 排队一个面向 emitter 的播放请求
    pub fn queue_play_on(&mut self, emitter_id: EmitterId, event_id: EventId) {
        self.command_buffer.queue_play_on(emitter_id, event_id);
    }

    /// 启动一个音乐图会话，使用图中声明的初始状态。
    pub fn play_music_graph(
        &mut self,
        graph_id: MusicGraphId,
    ) -> Result<MusicSessionId, FirewheelBackendError> {
        let session_id = self.runtime.play_music_graph(graph_id)?;
        self.sync_music_session_playback(session_id)?;
        Ok(session_id)
    }

    /// 启动一个音乐图会话，并显式指定初始状态。
    pub fn play_music_graph_in_state(
        &mut self,
        graph_id: MusicGraphId,
        initial_state: MusicStateId,
    ) -> Result<MusicSessionId, FirewheelBackendError> {
        let session_id = self
            .runtime
            .play_music_graph_in_state(graph_id, Some(initial_state))?;
        self.sync_music_session_playback(session_id)?;
        Ok(session_id)
    }

    /// 请求一个音乐会话切换到目标状态。
    pub fn request_music_state(
        &mut self,
        session_id: MusicSessionId,
        target_state: MusicStateId,
    ) -> Result<(), FirewheelBackendError> {
        let preview = self
            .runtime
            .preview_music_transition(session_id, target_state)?;
        self.save_music_session_resume_position(session_id)?;
        self.runtime.request_music_state(session_id, target_state)?;
        self.sync_music_session_playback(session_id)?;
        if let Some(transition) = preview {
            if transition.bridge_clip.is_none() {
                self.play_pending_transition_stinger(session_id, &transition)?;
            }
        }
        Ok(())
    }

    /// 通知后端：音乐会话已到达允许退出的切点。
    pub fn complete_music_exit(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<(), FirewheelBackendError> {
        let pending = self.runtime.music_status(session_id)?.pending_transition;
        self.save_music_session_resume_position(session_id)?;
        self.runtime.complete_music_exit(session_id)?;
        self.sync_music_session_playback(session_id)?;
        if let Some(transition) = pending {
            if transition.bridge_clip.is_none() {
                self.play_pending_transition_stinger(session_id, &transition)?;
            }
        }
        Ok(())
    }

    /// 通知后端：桥接片段已经结束，可以进入目标状态。
    pub fn complete_music_bridge(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<(), FirewheelBackendError> {
        let pending = self.runtime.music_status(session_id)?.pending_transition;
        self.runtime.complete_music_bridge(session_id)?;
        self.sync_music_session_playback(session_id)?;
        if let Some(transition) = pending {
            self.play_pending_transition_stinger(session_id, &transition)?;
        }
        Ok(())
    }

    /// 停止一个音乐会话。
    pub fn stop_music_session(
        &mut self,
        session_id: MusicSessionId,
        fade: Fade,
    ) -> Result<(), FirewheelBackendError> {
        self.save_music_session_resume_position(session_id)?;
        self.runtime.stop_music_session(session_id, fade)?;
        self.pending_music_playbacks.remove(&session_id);
        self.stop_music_session_workers(session_id, normalize_fade_duration_seconds(fade));
        self.stop_music_session_stinger_workers(session_id, normalize_fade_duration_seconds(fade));
        self.active_music_clips.remove(&session_id);
        self.active_music_tracks.remove(&session_id);
        self.update()?;
        Ok(())
    }

    /// 查询音乐会话当前对游戏侧可见的状态。
    pub fn music_status(
        &self,
        session_id: MusicSessionId,
    ) -> Result<MusicStatus, FirewheelBackendError> {
        Ok(self.runtime.music_status(session_id)?)
    }

    /// 当前音乐会话是否还在等待媒体资源就绪。
    pub fn music_session_pending_media(&self, session_id: MusicSessionId) -> bool {
        self.pending_music_playbacks.contains_key(&session_id)
    }

    /// 读取音乐会话当前的代表性播放头秒数。
    pub fn music_session_playhead_seconds(&self, session_id: MusicSessionId) -> Option<f64> {
        self.music_session_playhead(session_id)
            .map(|playhead| playhead.position_seconds)
    }

    /// 设置一个全局参数
    pub fn set_global_param(
        &mut self,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), FirewheelBackendError> {
        self.runtime.set_global_param(parameter_id, value)?;
        Ok(())
    }

    /// 排队一个全局参数更新请求
    pub fn queue_set_global_param(&mut self, parameter_id: ParameterId, value: ParameterValue) {
        self.command_buffer
            .queue_set_global_param(parameter_id, value);
    }

    /// 设置一个 emitter 参数
    pub fn set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), FirewheelBackendError> {
        self.runtime
            .set_emitter_param(emitter_id, parameter_id, value)?;
        Ok(())
    }

    /// 查询一个事件实例当前对游戏侧可见的播放状态。
    pub fn instance_state(&self, instance_id: EventInstanceId) -> EventInstanceState {
        if self.pending_playbacks.contains_key(&instance_id) {
            EventInstanceState::PendingMedia
        } else if self.instance_workers.contains_key(&instance_id) {
            EventInstanceState::Playing
        } else if self.runtime.active_plan(instance_id).is_some() {
            EventInstanceState::PendingMedia
        } else {
            EventInstanceState::Stopped
        }
    }

    /// 排队一个 emitter 参数更新请求
    pub fn queue_set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) {
        self.command_buffer
            .queue_set_emitter_param(emitter_id, parameter_id, value);
    }

    /// 排队一个停止实例请求
    pub fn queue_stop(&mut self, instance_id: EventInstanceId, fade: Fade) {
        self.command_buffer.queue_stop(instance_id, fade);
    }

    /// 取出当前所有待处理请求
    pub fn drain_requests(&mut self) -> Vec<FirewheelRequest> {
        self.command_buffer.drain()
    }

    /// 依次执行所有待处理请求, 遇到第一条错误立即返回
    pub fn apply_requests(&mut self) -> Result<Vec<FirewheelRequestResult>, FirewheelBackendError> {
        let requests = self.command_buffer.drain();
        let mut results = Vec::with_capacity(requests.len());

        for request in requests {
            results.push(self.apply_request(&request)?);
        }

        Ok(results)
    }

    /// 依次执行所有待处理请求, 单条失败不会中断整批处理
    pub fn apply_requests_isolated(&mut self) -> Vec<FirewheelRequestOutcome> {
        let requests = self.command_buffer.drain();

        requests
            .into_iter()
            .map(|request| {
                let result = self.apply_request(&request);
                FirewheelRequestOutcome { request, result }
            })
            .collect()
    }

    /// 停止一个事件实例。
    ///
    /// 当前最小实现只支持立即停止。非零 fade 先显式报错，避免制造已经支持淡出的假象。
    pub fn stop(
        &mut self,
        instance_id: EventInstanceId,
        fade: Fade,
    ) -> Result<(), FirewheelBackendError> {
        self.runtime.stop(instance_id, fade)?;
        self.pending_playbacks.remove(&instance_id);
        self.stop_event_instance_workers(instance_id, normalize_fade_duration_seconds(fade));
        self.update()?;
        Ok(())
    }

    /// 读取一个实例当前的代表性播放头。
    ///
    /// 如果这个实例绑定了多个 worker，则返回第一个 worker 的播放头，
    /// 并同时报告 worker 总数，供调用方决定是否需要更细粒度处理。
    pub fn instance_playhead(&self, instance_id: EventInstanceId) -> Option<InstancePlayhead> {
        let worker_ids = self.instance_workers.get(&instance_id)?;
        let worker_id = *worker_ids.first()?;
        let sample_rate = self.context.stream_info()?.sample_rate;
        let update_instant = self.context.audio_clock_instant();
        let state = self
            .sampler_pool
            .first_node_state::<SamplerState, _>(worker_id, &self.context)?;

        Some(InstancePlayhead {
            position_seconds: state
                .playhead_seconds_corrected(update_instant, sample_rate)
                .0,
            worker_count: worker_ids.len(),
        })
    }

    fn music_session_playhead(&self, session_id: MusicSessionId) -> Option<InstancePlayhead> {
        let worker_ids = self.music_session_workers.get(&session_id)?;
        let sample_rate = self.context.stream_info()?.sample_rate;
        let update_instant = self.context.audio_clock_instant();
        worker_ids.iter().find_map(|worker_id| {
            let state = self
                .sampler_pool
                .first_node_state::<SamplerState, _>(*worker_id, &self.context)?;

            Some(InstancePlayhead {
                position_seconds: state
                    .playhead_seconds_corrected(update_instant, sample_rate)
                    .0,
                worker_count: worker_ids.len(),
            })
        })
    }

    fn music_session_has_live_worker(&self, session_id: MusicSessionId) -> bool {
        let Some(worker_ids) = self.music_session_workers.get(&session_id) else {
            return false;
        };

        worker_ids.iter().any(|worker_id| {
            self.sampler_pool
                .first_node_state::<SamplerState, _>(*worker_id, &self.context)
                .is_some()
        })
    }

    fn music_session_local_playhead(
        &self,
        session_id: MusicSessionId,
    ) -> Result<Option<f64>, FirewheelBackendError> {
        let Some(playhead) = self.music_session_playhead(session_id) else {
            return Ok(None);
        };
        let Some(clip_id) = self.active_music_clips.get(&session_id).copied() else {
            return Ok(None);
        };

        Ok(Some(self.clip_local_position_seconds(
            clip_id,
            playhead.position_seconds,
        )?))
    }

    /// 把一个实例当前所有 worker 的播放头同步到指定秒数。
    pub fn seek_instance(
        &mut self,
        instance_id: EventInstanceId,
        position_seconds: f64,
    ) -> Result<bool, FirewheelBackendError> {
        let position_seconds = validate_playback_position_seconds(position_seconds)?;
        self.seek_instance_internal(instance_id, position_seconds, None)
    }

    /// 在未来音频时钟的某个时刻把实例播放头同步到指定秒数。
    pub fn seek_instance_after(
        &mut self,
        instance_id: EventInstanceId,
        position_seconds: f64,
        delay_seconds: f64,
    ) -> Result<bool, FirewheelBackendError> {
        let position_seconds = validate_playback_position_seconds(position_seconds)?;
        let delay_seconds = validate_schedule_delay_seconds(delay_seconds)?;
        let start_time = Some(self.event_instant_after_seconds(delay_seconds));
        self.seek_instance_internal(instance_id, position_seconds, start_time)
    }

    /// 读取当前修正后的音频时钟秒数。
    pub fn audio_clock_seconds(&self) -> f64 {
        self.context.audio_clock_corrected().seconds.0
    }

    fn seek_instance_internal(
        &mut self,
        instance_id: EventInstanceId,
        position_seconds: f64,
        start_time: Option<EventInstant>,
    ) -> Result<bool, FirewheelBackendError> {
        let worker_ids = self
            .instance_workers
            .get(&instance_id)
            .cloned()
            .unwrap_or_default();
        let mut changed = false;

        for worker_id in worker_ids {
            let Some(mut sampler) = self.sampler_pool.first_node(worker_id).cloned() else {
                continue;
            };

            sampler.start_from(PlayFrom::Seconds(position_seconds));
            changed |= self.sampler_pool.sync_worker_params(
                worker_id,
                &sampler,
                start_time,
                &mut self.context,
            );
        }

        if changed {
            self.update()?;
        }

        Ok(changed)
    }

    /// 推进 Firewheel 上下文
    pub fn update(&mut self) -> Result<(), FirewheelBackendError> {
        self.context
            .update()
            .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        self.drain_ready_streaming_assets();
        self.start_ready_pending_playbacks()?;
        self.start_ready_pending_music_playbacks()?;
        self.refresh_waiting_exit_cues()?;
        let poll_result = self.sampler_pool.poll(&self.context);
        for worker_id in poll_result.finished_workers {
            self.finish_worker(worker_id);
        }
        self.advance_pending_bridge_completions()?;
        self.advance_waiting_exit_cues()?;
        Ok(())
    }

    fn apply_request(
        &mut self,
        request: &FirewheelRequest,
    ) -> Result<FirewheelRequestResult, FirewheelBackendError> {
        if let FirewheelRequest::Stop { instance_id, fade } = request {
            self.stop(*instance_id, *fade)?;
            return Ok(FirewheelRequestResult::Stopped {
                instance_id: *instance_id,
            });
        }

        let result = self.runtime.apply_request(request)?;

        if let FirewheelRequestResult::Played { instance_id } = result {
            let plan = self
                .runtime
                .active_plan(instance_id)
                .cloned()
                .expect("active plan should exist right after play request");
            self.playback_plan(instance_id, &plan)?;
            Ok(FirewheelRequestResult::Played { instance_id })
        } else {
            Ok(result)
        }
    }

    fn playback_plan(
        &mut self,
        instance_id: EventInstanceId,
        plan: &PlaybackPlan,
    ) -> Result<(), FirewheelBackendError> {
        if !self.is_playback_plan_ready(plan) {
            self.pending_playbacks.insert(instance_id, plan.clone());
            return Ok(());
        }

        self.pending_playbacks.remove(&instance_id);
        self.instance_workers.remove(&instance_id);
        let bus_volume = self.runtime.active_bus_volume(instance_id).unwrap_or(1.0);

        for asset_id in &plan.asset_ids {
            self.play_asset(instance_id, *asset_id, bus_volume, None, None)?;
        }

        self.update()?;
        Ok(())
    }

    fn sync_music_session_playback(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<(), FirewheelBackendError> {
        let status = self.runtime.music_status(session_id)?;

        match status.phase {
            MusicPhase::Stopped => {
                self.pending_music_playbacks.remove(&session_id);
                self.pending_exit_cues.remove(&session_id);
                self.pending_bridge_completions.remove(&session_id);
                self.stop_music_session_workers(session_id, 0.0);
                self.stop_music_session_stinger_workers(session_id, 0.0);
                self.active_music_clips.remove(&session_id);
                self.active_music_tracks.remove(&session_id);
                self.update()?;
            }
            MusicPhase::WaitingExitCue => {
                self.pending_bridge_completions.remove(&session_id);
                if !self.ensure_waiting_exit_cue(session_id)? {
                    self.complete_music_exit(session_id)?;
                }
            }
            MusicPhase::Stable | MusicPhase::EnteringDestination => {
                self.pending_exit_cues.remove(&session_id);
                self.pending_bridge_completions.remove(&session_id);
                let resolved_music = self
                    .runtime
                    .resolve_music_playback(session_id, self.audio_clock_seconds())?;
                let clip_id = resolved_music.clip_id;
                let track_id = resolved_music.track_id;

                if self.active_music_clips.get(&session_id) == Some(&clip_id)
                    && self.active_music_tracks.get(&session_id).copied().flatten() == track_id
                    && self.music_session_has_live_worker(session_id)
                    && !self.pending_music_playbacks.contains_key(&session_id)
                {
                    return Ok(());
                }

                self.start_music_session_clip(session_id, resolved_music)?;
            }
            MusicPhase::PlayingBridge => {
                self.pending_exit_cues.remove(&session_id);
                let resolved_music = self
                    .runtime
                    .resolve_music_playback(session_id, self.audio_clock_seconds())?;
                let clip_id = resolved_music.clip_id;
                let track_id = resolved_music.track_id;

                if self.active_music_clips.get(&session_id) == Some(&clip_id)
                    && self.active_music_tracks.get(&session_id).copied().flatten() == track_id
                    && self.music_session_has_live_worker(session_id)
                    && !self.pending_music_playbacks.contains_key(&session_id)
                {
                    return Ok(());
                }

                self.start_music_session_clip(session_id, resolved_music)?;
            }
        }

        Ok(())
    }

    fn start_music_session_clip(
        &mut self,
        session_id: MusicSessionId,
        resolved_music: ResolvedMusicPlayback,
    ) -> Result<(), FirewheelBackendError> {
        let clip_id = resolved_music.clip_id;
        let track_id = resolved_music.track_id;
        let resolved = self.resolve_clip_playback(clip_id, resolved_music.entry_offset_seconds)?;

        if !self.prepare_asset_for_playback(resolved.asset_id)? {
            self.pending_music_playbacks
                .insert(session_id, PendingMusicPlayback { clip_id, track_id });
            self.active_music_clips.insert(session_id, clip_id);
            self.active_music_tracks.insert(session_id, track_id);
            return Ok(());
        }

        self.pending_music_playbacks.remove(&session_id);
        self.flush_finished_workers()?;
        let existing_worker_count = self
            .music_session_workers
            .get(&session_id)
            .map(|worker_ids| worker_ids.len())
            .unwrap_or(0);
        if existing_worker_count > 0 {
            self.stop_music_session_workers(session_id, 0.0);
            self.flush_finished_workers()?;
        }
        let schedule_internal_stop = !matches!(
            self.runtime.music_status(session_id),
            Ok(status) if status.phase == MusicPhase::PlayingBridge
        );
        self.play_music_clip_resolved(session_id, resolved, schedule_internal_stop)?;
        self.schedule_bridge_completion(session_id, resolved, self.audio_clock_seconds())?;
        self.active_music_clips.insert(session_id, clip_id);
        self.active_music_tracks.insert(session_id, track_id);
        self.update()?;
        Ok(())
    }

    fn play_music_clip_resolved(
        &mut self,
        session_id: MusicSessionId,
        resolved: ResolvedClipPlayback,
        schedule_internal_stop: bool,
    ) -> Result<(), FirewheelBackendError> {
        let worker_id = self.play_clip_worker(
            resolved.asset_id,
            1.0,
            resolved.start_from_seconds,
            resolved.repeat_mode,
            None,
        )?;
        self.attach_music_session_worker(session_id, worker_id);

        if schedule_internal_stop {
            if let Some(stop_after_seconds) = resolved.stop_after_seconds {
                let stop_time = Some(self.event_instant_after_seconds(stop_after_seconds));
                self.sampler_pool
                    .stop(worker_id, stop_time, &mut self.context);
            }
        }
        Ok(())
    }

    fn resolve_clip_playback(
        &self,
        clip_id: ClipId,
        entry_offset_seconds: f64,
    ) -> Result<ResolvedClipPlayback, FirewheelBackendError> {
        let clip = self
            .runtime
            .clip(clip_id)
            .ok_or(FirewheelBackendError::ClipNotLoaded(clip_id))?;
        let clip_base_seconds = clip
            .source_range
            .map(|range| range.start_seconds as f64)
            .unwrap_or(0.0);
        let clip_base_seconds = validate_playback_position_seconds(clip_base_seconds)?;
        let entry_offset_seconds = validate_playback_position_seconds(entry_offset_seconds)?;
        let start_from_seconds = clip_base_seconds + entry_offset_seconds;

        let stop_after_seconds = if let Some(range) = clip.source_range {
            let end_seconds = validate_playback_position_seconds(range.end_seconds as f64)?;
            if end_seconds < clip_base_seconds {
                return Err(FirewheelBackendError::InvalidClipRange(clip_id));
            }

            if end_seconds > start_from_seconds {
                Some(end_seconds - start_from_seconds)
            } else {
                Some(0.0)
            }
        } else {
            None
        };

        let repeat_mode = match clip.loop_range {
            Some(_) if clip.source_range.is_some() => {
                return Err(FirewheelBackendError::UnsupportedClipLoopRange(clip_id));
            }
            Some(_) => RepeatMode::RepeatEndlessly,
            None => RepeatMode::PlayOnce,
        };

        Ok(ResolvedClipPlayback {
            asset_id: clip.asset_id,
            start_from_seconds,
            stop_after_seconds,
            repeat_mode,
        })
    }

    fn prepare_asset_for_playback(
        &mut self,
        asset_id: Uuid,
    ) -> Result<bool, FirewheelBackendError> {
        self.drain_ready_streaming_assets();

        if self.sample_resources.contains_key(&asset_id) {
            return Ok(true);
        }

        let asset = self
            .known_bank_assets
            .get(&asset_id)
            .cloned()
            .ok_or(FirewheelBackendError::AssetNotRegistered(asset_id))?;

        if self.loading_streaming_assets.contains(&asset_id) {
            return Ok(false);
        }

        if asset.streaming == sonara_model::StreamingMode::Streaming {
            self.prewarm_streaming_bank_asset(&asset);
            return Ok(false);
        }

        self.decode_bank_asset(&asset)?;
        Ok(true)
    }

    fn play_clip_worker(
        &mut self,
        asset_id: Uuid,
        bus_volume: f32,
        start_from_seconds: f64,
        repeat_mode: RepeatMode,
        start_time: Option<EventInstant>,
    ) -> Result<WorkerID, FirewheelBackendError> {
        self.prepare_asset_for_playback(asset_id)?;
        let resource = self
            .sample_resources
            .get(&asset_id)
            .cloned()
            .ok_or(FirewheelBackendError::AssetNotRegistered(asset_id))?;
        let mut sampler = SamplerNode::default();
        sampler.set_sample(resource);
        sampler.repeat_mode = repeat_mode;
        sampler.start_from(PlayFrom::Seconds(validate_playback_position_seconds(
            start_from_seconds,
        )?));

        let worker = self.sampler_pool.new_worker(
            &sampler,
            start_time,
            true,
            &mut self.context,
            |fx_chain, cx| {
                let mut params = fx_chain.fx_chain.volume_pan;
                params.set_volume_linear(bus_volume);
                fx_chain
                    .fx_chain
                    .set_params(params, None, &fx_chain.node_ids, cx);
                fx_chain.fx_chain.volume_pan = params;
            },
        )?;
        if let Some(old_worker_id) = worker.old_worker_id {
            self.finish_worker(old_worker_id);
        }

        Ok(worker.worker_id)
    }

    fn play_asset(
        &mut self,
        instance_id: EventInstanceId,
        asset_id: Uuid,
        bus_volume: f32,
        start_from_seconds: Option<f64>,
        start_time: Option<EventInstant>,
    ) -> Result<(), FirewheelBackendError> {
        self.ensure_bank_asset_ready(asset_id)?;
        let resource = self
            .sample_resources
            .get(&asset_id)
            .cloned()
            .ok_or(FirewheelBackendError::AssetNotRegistered(asset_id))?;
        let mut sampler = SamplerNode::default();
        sampler.set_sample(resource);
        if let Some(start_from_seconds) = start_from_seconds {
            sampler.start_from(PlayFrom::Seconds(validate_playback_position_seconds(
                start_from_seconds,
            )?));
        } else {
            sampler.start_or_restart();
        }

        let worker = self.sampler_pool.new_worker(
            &sampler,
            start_time,
            true,
            &mut self.context,
            |fx_chain, cx| {
                let mut params = fx_chain.fx_chain.volume_pan;
                params.set_volume_linear(bus_volume);
                fx_chain
                    .fx_chain
                    .set_params(params, None, &fx_chain.node_ids, cx);
                fx_chain.fx_chain.volume_pan = params;
            },
        )?;
        self.attach_worker(instance_id, worker.worker_id);

        if let Some(old_worker_id) = worker.old_worker_id {
            self.finish_worker(old_worker_id);
        }

        Ok(())
    }

    /// 确保资源在真正播放前已经准备成 Firewheel 可消费的 sample resource。
    ///
    /// resident 媒体会在 bank 加载阶段提前完成,
    /// streaming 媒体优先等待后台预热结果, 只有兜底时才同步解码。
    fn ensure_bank_asset_ready(&mut self, asset_id: Uuid) -> Result<(), FirewheelBackendError> {
        self.drain_ready_streaming_assets();

        if self.sample_resources.contains_key(&asset_id) {
            return Ok(());
        }

        let asset = self
            .known_bank_assets
            .get(&asset_id)
            .cloned()
            .ok_or(FirewheelBackendError::AssetNotRegistered(asset_id))?;

        if self.loading_streaming_assets.contains(&asset.id) {
            return Ok(());
        }

        self.decode_bank_asset(&asset)
    }

    /// 把 streaming 资源的解码工作尽早移到后台线程里做, 避免第一次切状态时再卡主线程。
    fn prewarm_streaming_bank_asset(&mut self, asset: &BankAsset) {
        if self.sample_resources.contains_key(&asset.id)
            || !self.loading_streaming_assets.insert(asset.id)
        {
            return;
        }

        let tx = self.streaming_asset_tx.clone();
        let asset = asset.clone();
        thread::spawn(move || {
            let result = load_bank_asset_resource(&asset);
            let _ = tx.send(StreamingAssetLoadResult {
                asset_id: asset.id,
                result,
            });
        });
    }

    /// 把后台已经完成的 streaming 资源注册回主线程 backend 状态。
    fn drain_ready_streaming_assets(&mut self) {
        while let Ok(message) = self.streaming_asset_rx.try_recv() {
            self.loading_streaming_assets.remove(&message.asset_id);

            if let Ok(decoded) = message.result {
                self.register_sample_resource(message.asset_id, decoded.into());
            }
        }
    }

    fn is_playback_plan_ready(&mut self, plan: &PlaybackPlan) -> bool {
        for asset_id in &plan.asset_ids {
            if self.sample_resources.contains_key(asset_id) {
                continue;
            }

            let Some(asset) = self.known_bank_assets.get(asset_id).cloned() else {
                return false;
            };

            if asset.streaming == sonara_model::StreamingMode::Streaming {
                self.prewarm_streaming_bank_asset(&asset);
                return false;
            }

            return false;
        }

        true
    }

    /// 后台资源准备好之后, 在常规 update 阶段把之前挂起的实例真正启动。
    fn start_ready_pending_playbacks(&mut self) -> Result<(), FirewheelBackendError> {
        let pending_playbacks: Vec<_> = self
            .pending_playbacks
            .iter()
            .map(|(instance_id, plan)| (*instance_id, plan.clone()))
            .collect();
        let ready_instance_ids: Vec<_> = pending_playbacks
            .into_iter()
            .filter_map(|(instance_id, plan)| {
                self.is_playback_plan_ready(&plan).then_some(instance_id)
            })
            .collect();

        for instance_id in ready_instance_ids {
            let Some(plan) = self.pending_playbacks.remove(&instance_id) else {
                continue;
            };
            self.playback_plan(instance_id, &plan)?;
        }

        Ok(())
    }

    fn start_ready_pending_music_playbacks(&mut self) -> Result<(), FirewheelBackendError> {
        let pending_sessions: Vec<_> = self.pending_music_playbacks.keys().copied().collect();
        let mut started_any = false;

        for session_id in pending_sessions {
            let Some(pending) = self.pending_music_playbacks.get(&session_id).copied() else {
                continue;
            };
            let resolved_music = self
                .runtime
                .resolve_music_playback(session_id, self.audio_clock_seconds())?;
            if resolved_music.clip_id != pending.clip_id
                || resolved_music.track_id != pending.track_id
            {
                self.pending_music_playbacks.insert(
                    session_id,
                    PendingMusicPlayback {
                        clip_id: resolved_music.clip_id,
                        track_id: resolved_music.track_id,
                    },
                );
            }
            let resolved = self.resolve_clip_playback(
                resolved_music.clip_id,
                resolved_music.entry_offset_seconds,
            )?;

            if !self.prepare_asset_for_playback(resolved.asset_id)? {
                continue;
            }

            self.pending_music_playbacks.remove(&session_id);
            self.flush_finished_workers()?;
            let existing_worker_count = self
                .music_session_workers
                .get(&session_id)
                .map(|worker_ids| worker_ids.len())
                .unwrap_or(0);
            if existing_worker_count > 0 {
                self.stop_music_session_workers(session_id, 0.0);
                self.flush_finished_workers()?;
            }
            let schedule_internal_stop = !matches!(
                self.runtime.music_status(session_id),
                Ok(status) if status.phase == MusicPhase::PlayingBridge
            );
            self.play_music_clip_resolved(session_id, resolved, schedule_internal_stop)?;
            self.schedule_bridge_completion(session_id, resolved, self.audio_clock_seconds())?;
            self.active_music_clips
                .insert(session_id, resolved_music.clip_id);
            self.active_music_tracks
                .insert(session_id, resolved_music.track_id);
            started_any = true;
        }

        if started_any {
            self.context
                .update()
                .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        }

        Ok(())
    }

    fn ensure_waiting_exit_cue(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<bool, FirewheelBackendError> {
        if self.pending_exit_cues.contains_key(&session_id) {
            return Ok(true);
        }

        let Some(current_position_seconds) = self.music_session_local_playhead(session_id)? else {
            return Ok(true);
        };
        let Some(next_cue) = self
            .runtime
            .find_next_music_exit_cue(session_id, current_position_seconds)?
        else {
            return Ok(false);
        };
        let target_audio_time_seconds = self.target_audio_time_for_music_cue(
            session_id,
            current_position_seconds,
            next_cue.cue_position_seconds,
            next_cue.requires_wrap,
        )?;

        self.pending_exit_cues.insert(
            session_id,
            PendingExitCue {
                target_position_seconds: next_cue.cue_position_seconds,
                target_audio_time_seconds,
                waiting_for_wrap: next_cue.requires_wrap,
                last_position_seconds: current_position_seconds,
            },
        );
        Ok(true)
    }

    fn refresh_waiting_exit_cues(&mut self) -> Result<(), FirewheelBackendError> {
        let session_ids: HashSet<_> = self
            .pending_exit_cues
            .keys()
            .chain(self.pending_music_playbacks.keys())
            .chain(self.active_music_clips.keys())
            .copied()
            .collect();

        for session_id in session_ids {
            let waiting = matches!(
                self.runtime.music_status(session_id),
                Ok(status) if status.phase == MusicPhase::WaitingExitCue
            );
            if !waiting {
                self.pending_exit_cues.remove(&session_id);
                continue;
            }

            if self.pending_exit_cues.contains_key(&session_id) {
                continue;
            }

            if !self.ensure_waiting_exit_cue(session_id)? {
                self.complete_music_exit(session_id)?;
            }
        }

        Ok(())
    }

    fn schedule_bridge_completion(
        &mut self,
        session_id: MusicSessionId,
        resolved: ResolvedClipPlayback,
        start_audio_time_seconds: f64,
    ) -> Result<(), FirewheelBackendError> {
        let playing_bridge = matches!(
            self.runtime.music_status(session_id),
            Ok(status) if status.phase == MusicPhase::PlayingBridge
        );
        if !playing_bridge {
            self.pending_bridge_completions.remove(&session_id);
            return Ok(());
        }

        let Some(stop_after_seconds) = resolved.stop_after_seconds else {
            self.pending_bridge_completions.remove(&session_id);
            return Ok(());
        };

        self.pending_bridge_completions.insert(
            session_id,
            PendingBridgeCompletion {
                target_audio_time_seconds: start_audio_time_seconds + stop_after_seconds,
            },
        );
        Ok(())
    }

    fn advance_pending_bridge_completions(&mut self) -> Result<(), FirewheelBackendError> {
        let session_ids: Vec<_> = self.pending_bridge_completions.keys().copied().collect();
        let mut ready_sessions = Vec::new();
        let now_seconds = self.audio_clock_seconds();

        for session_id in session_ids {
            let Some(pending) = self.pending_bridge_completions.get(&session_id).copied() else {
                continue;
            };

            let playing_bridge = matches!(
                self.runtime.music_status(session_id),
                Ok(status) if status.phase == MusicPhase::PlayingBridge
            );
            if !playing_bridge {
                self.pending_bridge_completions.remove(&session_id);
                continue;
            }

            if now_seconds + MUSIC_SCHEDULE_EARLY_SECONDS >= pending.target_audio_time_seconds {
                ready_sessions.push(session_id);
            }
        }

        for session_id in ready_sessions {
            self.pending_bridge_completions.remove(&session_id);
            self.complete_music_bridge(session_id)?;
        }

        Ok(())
    }

    fn advance_waiting_exit_cues(&mut self) -> Result<(), FirewheelBackendError> {
        let session_ids: Vec<_> = self.pending_exit_cues.keys().copied().collect();
        let mut ready_sessions = Vec::new();
        let epsilon = 0.010;
        let now_seconds = self.audio_clock_seconds();

        for session_id in session_ids {
            let Some(mut pending) = self.pending_exit_cues.get(&session_id).copied() else {
                continue;
            };

            let waiting = matches!(
                self.runtime.music_status(session_id),
                Ok(status) if status.phase == MusicPhase::WaitingExitCue
            );
            if !waiting {
                self.pending_exit_cues.remove(&session_id);
                continue;
            }

            if let Some(target_audio_time_seconds) = pending.target_audio_time_seconds {
                if now_seconds + MUSIC_SCHEDULE_EARLY_SECONDS >= target_audio_time_seconds {
                    ready_sessions.push(session_id);
                    continue;
                }
            }

            let Some(current_position_seconds) = self.music_session_local_playhead(session_id)?
            else {
                continue;
            };

            if pending.waiting_for_wrap
                && current_position_seconds + epsilon < pending.last_position_seconds
            {
                pending.waiting_for_wrap = false;
            }

            if !pending.waiting_for_wrap
                && current_position_seconds + epsilon >= pending.target_position_seconds
            {
                ready_sessions.push(session_id);
            } else {
                pending.last_position_seconds = current_position_seconds;
                self.pending_exit_cues.insert(session_id, pending);
            }
        }

        for session_id in ready_sessions {
            self.pending_exit_cues.remove(&session_id);
            self.complete_music_exit(session_id)?;
        }

        Ok(())
    }

    fn attach_worker(&mut self, instance_id: EventInstanceId, worker_id: WorkerID) {
        self.worker_instances.insert(worker_id, instance_id);
        self.instance_workers
            .entry(instance_id)
            .or_default()
            .push(worker_id);
    }

    fn attach_music_session_worker(&mut self, session_id: MusicSessionId, worker_id: WorkerID) {
        self.worker_music_sessions.insert(worker_id, session_id);
        self.music_session_workers
            .entry(session_id)
            .or_default()
            .push(worker_id);
    }

    fn attach_music_stinger_worker(&mut self, session_id: MusicSessionId, worker_id: WorkerID) {
        self.worker_music_stingers.insert(worker_id, session_id);
        self.music_session_stinger_workers
            .entry(session_id)
            .or_default()
            .push(worker_id);
    }

    fn play_pending_transition_stinger(
        &mut self,
        session_id: MusicSessionId,
        transition: &sonara_runtime::PendingMusicTransition,
    ) -> Result<(), FirewheelBackendError> {
        let Some(clip_id) = transition.stinger_clip else {
            return Ok(());
        };

        let mut resolved = self.resolve_clip_playback(clip_id, 0.0)?;
        resolved.repeat_mode = RepeatMode::PlayOnce;

        if !self.prepare_asset_for_playback(resolved.asset_id)? {
            return Ok(());
        }

        let worker_id = self.play_clip_worker(
            resolved.asset_id,
            1.0,
            resolved.start_from_seconds,
            resolved.repeat_mode,
            None,
        )?;
        self.attach_music_stinger_worker(session_id, worker_id);

        if let Some(stop_after_seconds) = resolved.stop_after_seconds {
            let stop_time = Some(self.event_instant_after_seconds(stop_after_seconds));
            self.sampler_pool
                .stop(worker_id, stop_time, &mut self.context);
        }

        self.update()?;
        Ok(())
    }

    fn save_music_session_resume_position(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<bool, FirewheelBackendError> {
        let Some(playhead) = self.music_session_playhead(session_id) else {
            return Ok(false);
        };
        let Some(clip_id) = self.active_music_clips.get(&session_id).copied() else {
            return Ok(false);
        };
        let clip_local_seconds =
            self.clip_local_position_seconds(clip_id, playhead.position_seconds)?;
        Ok(self.runtime.save_music_session_resume_position(
            session_id,
            clip_local_seconds,
            self.audio_clock_seconds(),
        )?)
    }

    fn clip_local_position_seconds(
        &self,
        clip_id: ClipId,
        asset_position_seconds: f64,
    ) -> Result<f64, FirewheelBackendError> {
        let clip = self
            .runtime
            .clip(clip_id)
            .ok_or(FirewheelBackendError::ClipNotLoaded(clip_id))?;
        let clip_base_seconds = clip
            .source_range
            .map(|range| range.start_seconds as f64)
            .unwrap_or(0.0);
        let clip_base_seconds = validate_playback_position_seconds(clip_base_seconds)?;
        let asset_position_seconds = validate_playback_position_seconds(asset_position_seconds)?;

        Ok((asset_position_seconds - clip_base_seconds).max(0.0))
    }

    fn clip_duration_seconds(&self, clip_id: ClipId) -> Result<Option<f64>, FirewheelBackendError> {
        let clip = self
            .runtime
            .clip(clip_id)
            .ok_or(FirewheelBackendError::ClipNotLoaded(clip_id))?;
        let clip_base_seconds = clip
            .source_range
            .map(|range| range.start_seconds as f64)
            .unwrap_or(0.0);
        let clip_base_seconds = validate_playback_position_seconds(clip_base_seconds)?;

        if let Some(range) = clip.source_range {
            let end_seconds = validate_playback_position_seconds(range.end_seconds as f64)?;
            return Ok(Some((end_seconds - clip_base_seconds).max(0.0)));
        }

        let Some(resource) = self.sample_resources.get(&clip.asset_id) else {
            return Ok(None);
        };
        let Some(sample_rate) = resource.sample_rate() else {
            return Ok(None);
        };
        let sample_rate = f64::from(sample_rate.get());
        if sample_rate <= 0.0 {
            return Ok(None);
        }

        Ok(Some(
            resource.len_frames() as f64 / sample_rate - clip_base_seconds,
        ))
    }

    fn target_audio_time_for_music_cue(
        &self,
        session_id: MusicSessionId,
        current_position_seconds: f64,
        cue_position_seconds: f64,
        requires_wrap: bool,
    ) -> Result<Option<f64>, FirewheelBackendError> {
        let Some(clip_id) = self.active_music_clips.get(&session_id).copied() else {
            return Ok(None);
        };
        let delta_seconds = if requires_wrap {
            let Some(clip_duration_seconds) = self.clip_duration_seconds(clip_id)? else {
                return Ok(None);
            };
            (clip_duration_seconds - current_position_seconds).max(0.0) + cue_position_seconds
        } else {
            (cue_position_seconds - current_position_seconds).max(0.0)
        };

        Ok(Some(self.audio_clock_seconds() + delta_seconds))
    }

    fn stop_event_instance_workers(&mut self, instance_id: EventInstanceId, fade_seconds: f64) {
        let worker_ids = self
            .instance_workers
            .remove(&instance_id)
            .unwrap_or_default();
        for worker_id in worker_ids {
            self.worker_instances.remove(&worker_id);
            self.stop_worker(worker_id, fade_seconds);
        }
    }

    fn stop_music_session_workers(&mut self, session_id: MusicSessionId, fade_seconds: f64) {
        let worker_ids = self
            .music_session_workers
            .remove(&session_id)
            .unwrap_or_default();
        for worker_id in worker_ids {
            self.worker_music_sessions.remove(&worker_id);
            self.stop_worker(worker_id, fade_seconds);
        }
    }

    fn stop_music_session_stinger_workers(
        &mut self,
        session_id: MusicSessionId,
        fade_seconds: f64,
    ) {
        let worker_ids = self
            .music_session_stinger_workers
            .remove(&session_id)
            .unwrap_or_default();
        for worker_id in worker_ids {
            self.worker_music_stingers.remove(&worker_id);
            self.stop_worker(worker_id, fade_seconds);
        }
    }

    fn stop_worker(&mut self, worker_id: WorkerID, fade_seconds: f64) {
        if fade_seconds > 0.0 {
            self.set_worker_volume_linear(worker_id, 0.0, fade_seconds, None);
            let stop_time = Some(self.event_instant_after_seconds(fade_seconds));
            self.sampler_pool
                .stop(worker_id, stop_time, &mut self.context);
        } else {
            self.sampler_pool.stop(worker_id, None, &mut self.context);
        }
    }

    fn set_worker_volume_linear(
        &mut self,
        worker_id: WorkerID,
        volume_linear: f32,
        smooth_seconds: f64,
        start_time: Option<EventInstant>,
    ) -> bool {
        let Some(fx_state) = self.sampler_pool.fx_chain_mut(worker_id) else {
            return false;
        };

        let mut params = fx_state.fx_chain.volume_pan;
        params.smooth_seconds = smooth_seconds.max(0.0) as f32;
        params.set_volume_linear(volume_linear.max(0.0));
        fx_state
            .fx_chain
            .set_params(params, start_time, &fx_state.node_ids, &mut self.context);
        fx_state.fx_chain.volume_pan = params;
        true
    }

    fn flush_finished_workers(&mut self) -> Result<(), FirewheelBackendError> {
        self.context
            .update()
            .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        let poll_result = self.sampler_pool.poll(&self.context);
        for worker_id in poll_result.finished_workers {
            self.finish_worker(worker_id);
        }
        Ok(())
    }

    fn finish_worker(&mut self, worker_id: WorkerID) {
        if self
            .sampler_pool
            .first_node_state::<SamplerState, _>(worker_id, &self.context)
            .is_some()
        {
            return;
        }

        if let Some(instance_id) = self.worker_instances.remove(&worker_id) {
            if let Some(worker_ids) = self.instance_workers.get_mut(&instance_id) {
                worker_ids.retain(|id| *id != worker_id);

                if worker_ids.is_empty() {
                    self.instance_workers.remove(&instance_id);
                    let _ = self.runtime.stop(instance_id, Fade::IMMEDIATE);
                }
            }

            return;
        }

        if let Some(session_id) = self.worker_music_stingers.remove(&worker_id) {
            if let Some(worker_ids) = self.music_session_stinger_workers.get_mut(&session_id) {
                worker_ids.retain(|id| *id != worker_id);
                if worker_ids.is_empty() {
                    self.music_session_stinger_workers.remove(&session_id);
                }
            }
            return;
        }

        let Some(session_id) = self.worker_music_sessions.remove(&worker_id) else {
            return;
        };
        let mut bridge_finished = false;

        if let Some(worker_ids) = self.music_session_workers.get_mut(&session_id) {
            worker_ids.retain(|id| *id != worker_id);

            if worker_ids.is_empty() {
                self.music_session_workers.remove(&session_id);
                self.active_music_clips.remove(&session_id);
                self.active_music_tracks.remove(&session_id);
                bridge_finished = matches!(
                    self.runtime.music_status(session_id),
                    Ok(status) if status.phase == MusicPhase::PlayingBridge
                );
            }
        }

        if bridge_finished {
            let _ = self.complete_music_bridge(session_id);
        }
    }

    fn event_instant_after_seconds(&self, delay_seconds: f64) -> EventInstant {
        EventInstant::Seconds(
            self.context.audio_clock_corrected().seconds + DurationSeconds(delay_seconds),
        )
    }
}

/// 根据 compiled bank manifest 决定资源是否需要在 startup 阶段预解码。
fn should_preload_bank_asset(
    asset: &BankAsset,
    resident_media: &HashSet<Uuid>,
    streaming_media: &HashSet<Uuid>,
) -> bool {
    if resident_media.contains(&asset.id) {
        return true;
    }

    if streaming_media.contains(&asset.id) {
        return false;
    }

    asset.streaming != sonara_model::StreamingMode::Streaming
}

struct StreamingAssetLoadResult {
    asset_id: Uuid,
    result: Result<DecodedAudio, String>,
}

fn load_bank_asset_resource(asset: &BankAsset) -> Result<DecodedAudio, String> {
    let mut loader = symphonium::SymphoniumLoader::new();
    let target_sample_rate = asset
        .import_settings
        .target_sample_rate
        .and_then(NonZeroU32::new);
    load_audio_file(
        &mut loader,
        asset.source_path.as_std_path(),
        target_sample_rate,
        symphonium::ResampleQuality::default(),
    )
    .map_err(|error| error.to_string())
}

fn validate_playback_position_seconds(seconds: f64) -> Result<f64, FirewheelBackendError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(FirewheelBackendError::InvalidPlaybackPosition(seconds));
    }

    Ok(seconds)
}

fn validate_schedule_delay_seconds(seconds: f64) -> Result<f64, FirewheelBackendError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(FirewheelBackendError::InvalidScheduleDelay(seconds));
    }

    Ok(seconds)
}

fn normalize_fade_duration_seconds(fade: Fade) -> f64 {
    let duration_seconds = f64::from(fade.duration_seconds);
    if duration_seconds.is_finite() && duration_seconds > 0.0 {
        duration_seconds
    } else {
        0.0
    }
}
