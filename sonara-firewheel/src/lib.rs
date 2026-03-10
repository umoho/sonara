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
use sonara_model::{AudioAsset, Bank, Event, EventId};
use sonara_runtime::{EmitterId, EventInstanceId, PlaybackPlan, RuntimeError, SonaraRuntime};
use thiserror::Error;
use uuid::Uuid;

/// Firewheel 后端错误
#[derive(Debug, Error)]
pub enum FirewheelBackendError {
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("bank `{0:?}` 引用了未提供定义的资源 `{1}`")]
    MissingAssetDefinition(sonara_model::BankId, Uuid),
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

/// 基于 Firewheel 的最小 one-shot 播放后端
pub struct FirewheelBackend {
    runtime: SonaraRuntime,
    context: FirewheelContext,
    sampler_pool: SamplerPoolVolumePan,
    sample_resources: HashMap<Uuid, ArcGc<dyn SampleResource>>,
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
        assets: Vec<AudioAsset>,
    ) -> Result<(), FirewheelBackendError> {
        let asset_by_id: HashMap<Uuid, &AudioAsset> =
            assets.iter().map(|asset| (asset.id, asset)).collect();

        // 先注册 bank 引用到的资源, 让后续 play 路径只消费已准备好的 SampleResource.
        for asset_id in bank
            .resident_media
            .iter()
            .chain(bank.streaming_media.iter())
        {
            let asset = asset_by_id.get(asset_id).copied().ok_or(
                FirewheelBackendError::MissingAssetDefinition(bank.id, *asset_id),
            )?;
            self.register_audio_asset(asset)?;
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

    /// 推进 Firewheel 上下文
    pub fn update(&mut self) -> Result<(), FirewheelBackendError> {
        self.context
            .update()
            .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        self.sampler_pool.poll(&self.context);
        Ok(())
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
