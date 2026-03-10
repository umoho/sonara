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
use firewheel_pool::{NewWorkerError, SamplerPoolVolumePan};
use firewheel_symphonium::load_audio_file;
use sonara_build::BuildError;
use sonara_model::{AudioAsset, Bank, Event, EventId, ParameterId, ParameterValue};
use sonara_runtime::{
    EmitterId, EventInstanceId, PlaybackPlan, RuntimeError, RuntimeRequest, RuntimeRequestResult,
    SonaraRuntime,
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
}

/// Firewheel backend 可消费的一条最小请求
pub type FirewheelRequest = RuntimeRequest;

/// Firewheel backend 执行请求后的结果
pub type FirewheelRequestResult = RuntimeRequestResult;

/// Firewheel backend 在隔离模式下处理单条请求后的结果
#[derive(Debug)]
pub struct FirewheelRequestOutcome {
    pub request: FirewheelRequest,
    pub result: Result<FirewheelRequestResult, FirewheelBackendError>,
}

/// 基于 Firewheel 的最小 one-shot 播放后端
pub struct FirewheelBackend {
    runtime: SonaraRuntime,
    context: FirewheelContext,
    sampler_pool: SamplerPoolVolumePan,
    sample_resources: HashMap<Uuid, ArcGc<dyn SampleResource>>,
    pending_requests: Vec<FirewheelRequest>,
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
            pending_requests: Vec::new(),
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
        // 先注册 bank 引用到的资源, 让后续 play 路径只消费已准备好的 SampleResource.
        for asset in &bank.assets {
            let audio_asset = AudioAsset {
                id: asset.id,
                name: asset.name.clone(),
                source_path: asset.source_path.clone(),
                import_settings: asset.import_settings.clone(),
                streaming: asset.streaming,
                loop_region: None,
                analysis: None,
            };
            self.register_audio_asset(&audio_asset)?;
        }

        self.runtime.load_bank(bank, events)?;
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

    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, FirewheelBackendError> {
        let instance_id = self.runtime.play(event_id)?;
        let plan = self
            .runtime
            .active_plan(instance_id)
            .cloned()
            .expect("active plan should exist right after play");
        self.playback_plan(&plan)?;
        Ok(instance_id)
    }

    /// 排队一个未绑定 emitter 的播放请求
    pub fn queue_play(&mut self, event_id: EventId) {
        self.pending_requests
            .push(FirewheelRequest::Play { event_id });
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
        self.playback_plan(&plan)?;
        Ok(instance_id)
    }

    /// 排队一个面向 emitter 的播放请求
    pub fn queue_play_on(&mut self, emitter_id: EmitterId, event_id: EventId) {
        self.pending_requests.push(FirewheelRequest::PlayOnEmitter {
            emitter_id,
            event_id,
        });
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
        self.pending_requests
            .push(FirewheelRequest::SetGlobalParam {
                parameter_id,
                value,
            });
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
        self.pending_requests
            .push(FirewheelRequest::SetEmitterParam {
                emitter_id,
                parameter_id,
                value,
            });
    }

    /// 取出当前所有待处理请求
    pub fn drain_requests(&mut self) -> Vec<FirewheelRequest> {
        self.pending_requests.drain(..).collect()
    }

    /// 依次执行所有待处理请求, 遇到第一条错误立即返回
    pub fn apply_requests(&mut self) -> Result<Vec<FirewheelRequestResult>, FirewheelBackendError> {
        let requests = self.drain_requests();
        let mut results = Vec::with_capacity(requests.len());

        for request in requests {
            results.push(self.apply_request(&request)?);
        }

        Ok(results)
    }

    /// 依次执行所有待处理请求, 单条失败不会中断整批处理
    pub fn apply_requests_isolated(&mut self) -> Vec<FirewheelRequestOutcome> {
        self.drain_requests()
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
        self.sampler_pool.poll(&self.context);
        Ok(())
    }

    fn apply_request(
        &mut self,
        request: &FirewheelRequest,
    ) -> Result<FirewheelRequestResult, FirewheelBackendError> {
        let result = self.runtime.apply_request(request)?;

        if let FirewheelRequestResult::Played { instance_id } = result {
            let plan = self
                .runtime
                .active_plan(instance_id)
                .cloned()
                .expect("active plan should exist right after play request");
            self.playback_plan(&plan)?;
            Ok(FirewheelRequestResult::Played { instance_id })
        } else {
            Ok(result)
        }
    }

    fn playback_plan(&mut self, plan: &PlaybackPlan) -> Result<(), FirewheelBackendError> {
        for asset_id in &plan.asset_ids {
            self.play_asset(*asset_id)?;
        }

        self.update()?;
        Ok(())
    }

    fn play_asset(&mut self, asset_id: Uuid) -> Result<(), FirewheelBackendError> {
        let resource = self
            .sample_resources
            .get(&asset_id)
            .cloned()
            .ok_or(FirewheelBackendError::AssetNotRegistered(asset_id))?;
        let mut sampler = SamplerNode::default();
        sampler.set_sample(resource);
        sampler.start_or_restart();

        self.sampler_pool
            .new_worker(&sampler, true, &mut self.context, |_fx_chain, _cx| {})?;

        Ok(())
    }
}
