// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::HashSet,
    num::{NonZeroU32, NonZeroUsize},
    thread,
};

use firewheel::{
    collector::ArcGc,
    sample_resource::{InterleavedResourceF32, SampleResource},
};
use firewheel_symphonium::{DecodedAudio, load_audio_file};
use sonara_build::CompiledBankPackage;
use sonara_model::{
    AudioAsset, Bank, BankAsset, BankId, BankManifest, Bus, Clip, Event, MusicGraph, ResumeSlot,
    Snapshot, SyncDomain,
};
use uuid::Uuid;

use crate::{
    backend::FirewheelBackend,
    error::FirewheelBackendError,
    types::{PendingMusicPlayback, StreamingAssetLoadResult},
};

impl FirewheelBackend {
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

    pub(crate) fn decode_bank_asset(
        &mut self,
        asset: &BankAsset,
    ) -> Result<(), FirewheelBackendError> {
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

    /// 确保资源在真正播放前已经准备成 Firewheel 可消费的 sample resource。
    ///
    /// resident 媒体会在 bank 加载阶段提前完成,
    /// streaming 媒体优先等待后台预热结果, 只有兜底时才同步解码。
    pub(crate) fn ensure_bank_asset_ready(
        &mut self,
        asset_id: Uuid,
    ) -> Result<(), FirewheelBackendError> {
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
    pub(crate) fn prewarm_streaming_bank_asset(&mut self, asset: &BankAsset) {
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
    pub(crate) fn drain_ready_streaming_assets(&mut self) {
        while let Ok(message) = self.streaming_asset_rx.try_recv() {
            self.loading_streaming_assets.remove(&message.asset_id);

            if let Ok(decoded) = message.result {
                self.register_sample_resource(message.asset_id, decoded.into());
            }
        }
    }

    pub(crate) fn is_playback_plan_ready(&mut self, plan: &sonara_runtime::PlaybackPlan) -> bool {
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
    pub(crate) fn start_ready_pending_playbacks(&mut self) -> Result<(), FirewheelBackendError> {
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

    pub(crate) fn start_ready_pending_music_playbacks(
        &mut self,
    ) -> Result<(), FirewheelBackendError> {
        let pending_sessions: Vec<_> = self.pending_music_playbacks.keys().copied().collect();
        let mut started_any = false;

        for session_id in pending_sessions {
            let Some(pending) = self.pending_music_playbacks.get(&session_id).cloned() else {
                continue;
            };
            let resolved_music = self
                .runtime
                .resolve_music_playback(session_id, self.audio_clock_seconds())?;
            let resolved_playbacks = self
                .runtime
                .resolve_music_node_playbacks(session_id, self.audio_clock_seconds())?;
            if resolved_music.clip_id != pending.primary_clip_id
                || resolved_music.track_id != pending.primary_track_id
                || resolved_playbacks != pending.playbacks
            {
                self.pending_music_playbacks.insert(
                    session_id,
                    PendingMusicPlayback {
                        primary_clip_id: resolved_music.clip_id,
                        primary_track_id: resolved_music.track_id,
                        playbacks: resolved_playbacks.clone(),
                    },
                );
            }

            let mut all_ready = true;
            for playback in &resolved_playbacks {
                let resolved =
                    self.resolve_clip_playback(playback.clip_id, playback.entry_offset_seconds)?;
                if !self.prepare_asset_for_playback(resolved.asset_id)? {
                    all_ready = false;
                }
            }

            if !all_ready {
                continue;
            }

            self.pending_music_playbacks.remove(&session_id);
            self.start_music_session_playbacks(session_id, resolved_music, resolved_playbacks)?;
            started_any = true;
        }

        if started_any {
            self.context
                .update()
                .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        }

        Ok(())
    }

    pub(crate) fn prepare_asset_for_playback(
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
