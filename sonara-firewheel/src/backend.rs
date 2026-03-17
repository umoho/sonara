// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{HashMap, HashSet},
    sync::mpsc,
};

use firewheel::{
    FirewheelConfig, FirewheelContext, collector::ArcGc, cpal::CpalConfig,
    nodes::sampler::SamplerNode, sample_resource::SampleResource,
};
use firewheel_pool::{SamplerPoolVolumePan, WorkerID};
use sonara_model::{BankAsset, BusId, ClipId, ParameterId, ParameterValue, TrackId};
use sonara_runtime::{
    EmitterId, EventInstanceId, EventInstanceState, Fade, MusicSessionId, PlaybackPlan,
    RuntimeCommandBuffer, RuntimeRequest, SonaraRuntime,
};
use uuid::Uuid;

use crate::{
    error::FirewheelBackendError,
    types::{
        FirewheelRequest, FirewheelRequestOutcome, FirewheelRequestResult, PendingExitCue,
        PendingMusicPlayback, PendingNodeCompletion, StreamingAssetLoadResult,
    },
};

/// 基于 Firewheel 的最小 one-shot 播放后端
pub struct FirewheelBackend {
    pub(crate) runtime: SonaraRuntime,
    pub(crate) context: FirewheelContext,
    pub(crate) sampler_pool: SamplerPoolVolumePan,
    pub(crate) known_bank_assets: HashMap<Uuid, BankAsset>,
    pub(crate) loading_streaming_assets: HashSet<Uuid>,
    pub(crate) pending_playbacks: HashMap<EventInstanceId, PlaybackPlan>,
    pub(crate) pending_music_playbacks: HashMap<MusicSessionId, PendingMusicPlayback>,
    pub(crate) pending_exit_cues: HashMap<MusicSessionId, PendingExitCue>,
    pub(crate) pending_node_completions: HashMap<MusicSessionId, PendingNodeCompletion>,
    pub(crate) sample_resources: HashMap<Uuid, ArcGc<dyn SampleResource>>,
    pub(crate) streaming_asset_tx: mpsc::Sender<StreamingAssetLoadResult>,
    pub(crate) streaming_asset_rx: mpsc::Receiver<StreamingAssetLoadResult>,
    pub(crate) instance_workers: HashMap<EventInstanceId, Vec<WorkerID>>,
    pub(crate) worker_instances: HashMap<WorkerID, EventInstanceId>,
    pub(crate) worker_buses: HashMap<WorkerID, BusId>,
    pub(crate) music_session_workers: HashMap<MusicSessionId, Vec<WorkerID>>,
    pub(crate) music_session_track_workers:
        HashMap<MusicSessionId, HashMap<TrackId, Vec<WorkerID>>>,
    pub(crate) worker_music_sessions: HashMap<WorkerID, MusicSessionId>,
    pub(crate) worker_music_tracks: HashMap<WorkerID, TrackId>,
    pub(crate) active_music_clips: HashMap<MusicSessionId, ClipId>,
    pub(crate) active_music_tracks: HashMap<MusicSessionId, Option<TrackId>>,
    pub(crate) active_music_binding_clips: HashMap<MusicSessionId, HashMap<TrackId, ClipId>>,
    pub(crate) command_buffer: RuntimeCommandBuffer,
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
            pending_node_completions: HashMap::new(),
            sample_resources: HashMap::new(),
            streaming_asset_tx,
            streaming_asset_rx,
            instance_workers: HashMap::new(),
            worker_instances: HashMap::new(),
            worker_buses: HashMap::new(),
            music_session_workers: HashMap::new(),
            music_session_track_workers: HashMap::new(),
            worker_music_sessions: HashMap::new(),
            worker_music_tracks: HashMap::new(),
            active_music_clips: HashMap::new(),
            active_music_tracks: HashMap::new(),
            active_music_binding_clips: HashMap::new(),
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

    /// 设置一个全局参数
    pub fn set_global_param(
        &mut self,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), FirewheelBackendError> {
        self.runtime.set_global_param(parameter_id, value)?;
        Ok(())
    }

    /// 设置某个 bus 的 live gain。
    pub fn set_bus_gain(&mut self, bus_id: BusId, gain: f32) -> Result<(), FirewheelBackendError> {
        self.runtime.set_bus_gain(bus_id, gain)?;
        let _ = self.sync_live_bus_gains();
        self.update()?;
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

    /// 推进 Firewheel 上下文
    pub fn update(&mut self) -> Result<(), FirewheelBackendError> {
        self.context
            .update()
            .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        self.drain_ready_streaming_assets();
        self.start_ready_pending_playbacks()?;
        self.start_ready_pending_music_playbacks()?;
        self.refresh_waiting_exit_cues()?;
        if self.sync_live_bus_gains() {
            self.context
                .update()
                .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        }
        let poll_result = self.sampler_pool.poll(&self.context);
        for worker_id in poll_result.finished_workers {
            self.finish_worker(worker_id);
        }
        self.advance_pending_node_completions()?;
        self.advance_waiting_exit_cues()?;
        Ok(())
    }

    pub(crate) fn apply_request(
        &mut self,
        request: &FirewheelRequest,
    ) -> Result<FirewheelRequestResult, FirewheelBackendError> {
        if let RuntimeRequest::Stop { instance_id, fade } = request {
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

    /// 读取当前修正后的音频时钟秒数。
    pub fn audio_clock_seconds(&self) -> f64 {
        self.context.audio_clock_corrected().seconds.0
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
}
