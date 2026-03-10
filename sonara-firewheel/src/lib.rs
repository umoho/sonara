//! Firewheel 后端适配层

use std::{
    collections::HashMap,
    num::{NonZeroU32, NonZeroUsize},
};

use firewheel::{
    FirewheelConfig, FirewheelContext,
    collector::ArcGc,
    cpal::CpalConfig,
    nodes::sampler::SamplerNode,
    sample_resource::{InterleavedResourceF32, SampleResource},
};
use firewheel_pool::{NewWorkerError, SamplerPoolVolumePan, WorkerID};
use firewheel_symphonium::load_audio_file;
use sonara_build::BuildError;
use sonara_model::{
    AudioAsset, Bank, BankAsset, BankManifest, Event, EventId, ParameterId, ParameterValue,
};
use sonara_runtime::{
    AudioCommandOutcome, EmitterId, EventInstanceId, Fade, PlaybackPlan, RuntimeCommandBuffer,
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
    #[error("Firewheel backend 暂不支持非立即 stop, fade={0} 秒")]
    UnsupportedFade(f32),
}

/// Firewheel backend 可消费的一条最小请求
pub type FirewheelRequest = RuntimeRequest;

/// Firewheel backend 执行请求后的结果
pub type FirewheelRequestResult = RuntimeRequestResult;

/// Firewheel backend 在隔离模式下处理单条请求后的结果
pub type FirewheelRequestOutcome =
    AudioCommandOutcome<FirewheelRequest, FirewheelRequestResult, FirewheelBackendError>;

/// 基于 Firewheel 的最小 one-shot 播放后端
pub struct FirewheelBackend {
    runtime: SonaraRuntime,
    context: FirewheelContext,
    sampler_pool: SamplerPoolVolumePan,
    sample_resources: HashMap<Uuid, ArcGc<dyn SampleResource>>,
    instance_workers: HashMap<EventInstanceId, Vec<WorkerID>>,
    worker_instances: HashMap<WorkerID, EventInstanceId>,
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

        Ok(Self {
            runtime,
            context,
            sampler_pool,
            sample_resources: HashMap::new(),
            instance_workers: HashMap::new(),
            worker_instances: HashMap::new(),
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
        self.register_bank_manifest(&bank.manifest)?;

        self.runtime.load_bank(bank, events)?;
        Ok(())
    }

    /// 注册一个 compiled bank manifest 引用到的所有资源。
    pub fn register_bank_manifest(
        &mut self,
        manifest: &BankManifest,
    ) -> Result<(), FirewheelBackendError> {
        for asset in &manifest.assets {
            self.register_bank_asset(asset)?;
        }

        Ok(())
    }

    /// 注册一个 compiled bank asset。
    ///
    /// 这条路径直接面向 bank 编译产物, 避免 backend 继续依赖 authoring 语义。
    pub fn register_bank_asset(&mut self, asset: &BankAsset) -> Result<(), FirewheelBackendError> {
        let mut loader = symphonium::SymphoniumLoader::new();
        let target_sample_rate = asset
            .import_settings
            .target_sample_rate
            .and_then(NonZeroU32::new);
        let decoded = load_audio_file(
            &mut loader,
            asset.source_path.as_std_path(),
            target_sample_rate,
            symphonium::ResampleQuality::default(),
        )
        .map_err(|error| FirewheelBackendError::DecodeAsset(asset.id, error.to_string()))?;

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
        self.register_bank_asset(&BankAsset {
            id: asset.id,
            name: asset.name.clone(),
            source_path: asset.source_path.clone(),
            import_settings: asset.import_settings.clone(),
            streaming: asset.streaming,
        })
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
        if fade.duration_seconds != 0.0 {
            return Err(FirewheelBackendError::UnsupportedFade(
                fade.duration_seconds,
            ));
        }

        self.runtime.stop(instance_id, fade)?;

        let worker_ids = self
            .instance_workers
            .remove(&instance_id)
            .unwrap_or_default();
        for worker_id in worker_ids {
            self.worker_instances.remove(&worker_id);
            self.sampler_pool.stop(worker_id, &mut self.context);
        }

        self.update()?;
        Ok(())
    }

    /// 推进 Firewheel 上下文
    pub fn update(&mut self) -> Result<(), FirewheelBackendError> {
        self.context
            .update()
            .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        let poll_result = self.sampler_pool.poll(&self.context);
        for worker_id in poll_result.finished_workers {
            self.finish_worker(worker_id);
        }
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
        self.instance_workers.remove(&instance_id);

        for asset_id in &plan.asset_ids {
            self.play_asset(instance_id, *asset_id)?;
        }

        self.update()?;
        Ok(())
    }

    fn play_asset(
        &mut self,
        instance_id: EventInstanceId,
        asset_id: Uuid,
    ) -> Result<(), FirewheelBackendError> {
        let resource = self
            .sample_resources
            .get(&asset_id)
            .cloned()
            .ok_or(FirewheelBackendError::AssetNotRegistered(asset_id))?;
        let mut sampler = SamplerNode::default();
        sampler.set_sample(resource);
        sampler.start_or_restart();

        let worker =
            self.sampler_pool
                .new_worker(&sampler, true, &mut self.context, |_fx_chain, _cx| {})?;
        self.attach_worker(instance_id, worker.worker_id);

        if let Some(old_worker_id) = worker.old_worker_id {
            self.finish_worker(old_worker_id);
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

    fn finish_worker(&mut self, worker_id: WorkerID) {
        let Some(instance_id) = self.worker_instances.remove(&worker_id) else {
            return;
        };

        if let Some(worker_ids) = self.instance_workers.get_mut(&instance_id) {
            worker_ids.retain(|id| *id != worker_id);

            if worker_ids.is_empty() {
                self.instance_workers.remove(&instance_id);
                let _ = self.runtime.stop(instance_id, Fade::IMMEDIATE);
            }
        }
    }
}
