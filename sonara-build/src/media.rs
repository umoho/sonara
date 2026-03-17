// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;

use sonara_model::{AudioAsset, Bank, BankAsset, StreamingMode};
use uuid::Uuid;

pub(crate) enum ResolvedMediaResidency {
    Resident,
    Streaming,
}

pub(crate) fn add_bank_asset_to_manifest(bank: &mut Bank, asset: &AudioAsset) {
    if !bank
        .manifest
        .assets
        .iter()
        .any(|bank_asset| bank_asset.id == asset.id)
    {
        bank.manifest.assets.push(BankAsset {
            id: asset.id,
            name: asset.name.clone(),
            source_path: asset.source_path.clone(),
            import_settings: asset.import_settings.clone(),
            streaming: asset.streaming,
        });
    }
}

pub(crate) fn finalize_bank_manifest_media(
    bank: &mut Bank,
    auto_assets_used_by_one_shot: &HashSet<Uuid>,
) {
    // `Auto` 先给一个最小可落地规则:
    // 只被 `Persistent` 事件引用的资源按 streaming 导出,
    // 只要被 `OneShot` 引用过, 仍然按 resident 处理, 避免把短音效误分流。
    let mut resident_media = HashSet::new();
    let mut streaming_media = HashSet::new();
    for asset in &bank.manifest.assets {
        match resolve_media_residency(asset, &auto_assets_used_by_one_shot) {
            ResolvedMediaResidency::Resident => {
                resident_media.insert(asset.id);
                streaming_media.remove(&asset.id);
            }
            ResolvedMediaResidency::Streaming => {
                if !resident_media.contains(&asset.id) {
                    streaming_media.insert(asset.id);
                }
            }
        }
    }

    bank.manifest.resident_media = resident_media.into_iter().collect();
    bank.manifest.streaming_media = streaming_media.into_iter().collect();
    bank.manifest.assets.sort_by(|a, b| a.id.cmp(&b.id));
    bank.manifest.resident_media.sort_unstable();
    bank.manifest.streaming_media.sort_unstable();
}

pub(crate) fn resolve_media_residency(
    asset: &BankAsset,
    auto_assets_used_by_one_shot: &HashSet<Uuid>,
) -> ResolvedMediaResidency {
    match asset.streaming {
        StreamingMode::Resident => ResolvedMediaResidency::Resident,
        StreamingMode::Streaming => ResolvedMediaResidency::Streaming,
        StreamingMode::Auto => {
            if auto_assets_used_by_one_shot.contains(&asset.id) {
                ResolvedMediaResidency::Resident
            } else {
                ResolvedMediaResidency::Streaming
            }
        }
    }
}
