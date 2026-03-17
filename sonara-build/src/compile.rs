// SPDX-License-Identifier: MPL-2.0

use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;
use sonara_model::{
    AudioAsset, AuthoringProject, Bank, BankDefinition, Bus, Clip, ClipId, Event, EventId,
    MusicGraph, MusicGraphId, Parameter, ParameterId, ResumeSlot, ResumeSlotId, Snapshot,
    SnapshotId, StreamingMode, SyncDomain, SyncDomainId,
};
use uuid::Uuid;

use crate::{
    error::{BuildError, ExportBankError},
    media::{add_bank_asset_to_manifest, finalize_bank_manifest_media},
    package::CompiledBankPackage,
    validate::{
        collect_event_asset_ids, push_unique, validate_event, validate_event_against_parameters,
        validate_music_graph,
    },
};

/// 根据事件和资源列表构建最小 bank 定义
pub fn build_bank(
    name: impl Into<SmolStr>,
    events: &[Event],
    assets: &[AudioAsset],
) -> Result<Bank, BuildError> {
    let mut bank = Bank::new(name);
    let asset_by_id: HashMap<Uuid, &AudioAsset> =
        assets.iter().map(|asset| (asset.id, asset)).collect();
    let mut auto_assets_used_by_one_shot = HashSet::new();

    for event in events {
        validate_event(event)?;
        bank.objects.events.push(event.id);

        for asset_id in collect_event_asset_ids(event) {
            let asset = asset_by_id
                .get(&asset_id)
                .ok_or(BuildError::MissingAudioAsset)?;

            if asset.streaming == StreamingMode::Auto
                && event.kind != sonara_model::EventKind::Persistent
            {
                auto_assets_used_by_one_shot.insert(asset_id);
            }

            add_bank_asset_to_manifest(&mut bank, asset);
        }
    }

    finalize_bank_manifest_media(&mut bank, &auto_assets_used_by_one_shot);

    Ok(bank)
}

/// 根据 authoring 项目里的 bank 定义构建一个 runtime bank。
pub fn build_bank_from_definition(
    definition: &BankDefinition,
    project: &AuthoringProject,
) -> Result<Bank, BuildError> {
    Ok(compile_bank_definition(definition, project)?.bank)
}

/// 根据 authoring 项目里的 bank 定义编译一份完整 bank 载荷。
pub fn compile_bank_definition(
    definition: &BankDefinition,
    project: &AuthoringProject,
) -> Result<CompiledBankPackage, BuildError> {
    let asset_by_id: HashMap<Uuid, &AudioAsset> = project
        .assets
        .iter()
        .map(|asset| (asset.id, asset))
        .collect();
    let event_by_id: HashMap<EventId, &Event> = project
        .events
        .iter()
        .map(|event| (event.id, event))
        .collect();
    let clip_by_id: HashMap<ClipId, &Clip> =
        project.clips.iter().map(|clip| (clip.id, clip)).collect();
    let resume_slot_by_id: HashMap<ResumeSlotId, &ResumeSlot> = project
        .resume_slots
        .iter()
        .map(|slot| (slot.id, slot))
        .collect();
    let sync_domain_by_id: HashMap<SyncDomainId, &SyncDomain> = project
        .sync_domains
        .iter()
        .map(|domain| (domain.id, domain))
        .collect();
    let music_graph_by_id: HashMap<MusicGraphId, &MusicGraph> = project
        .music_graphs
        .iter()
        .map(|graph| (graph.id, graph))
        .collect();
    let bus_by_id: HashMap<_, &Bus> = project.buses.iter().map(|bus| (bus.id, bus)).collect();
    let snapshot_by_id: HashMap<SnapshotId, &Snapshot> = project
        .snapshots
        .iter()
        .map(|snapshot| (snapshot.id, snapshot))
        .collect();
    let parameter_by_id: HashMap<ParameterId, &Parameter> = project
        .parameters
        .iter()
        .map(|parameter| (parameter.id(), parameter))
        .collect();

    let mut events = Vec::with_capacity(definition.events.len());
    let mut buses = Vec::with_capacity(definition.buses.len());
    let mut snapshots = Vec::with_capacity(definition.snapshots.len());
    let mut clips = Vec::new();
    let mut resume_slots = Vec::new();
    let mut sync_domains = Vec::new();
    let mut music_graphs = Vec::with_capacity(definition.music_graphs.len());
    let mut clip_ids = Vec::new();
    let mut resume_slot_ids = Vec::new();
    let mut sync_domain_ids = Vec::new();
    let mut auto_assets_used_by_one_shot = HashSet::new();

    for event_id in &definition.events {
        let event = event_by_id
            .get(event_id)
            .ok_or(BuildError::MissingEventDefinition)?;
        validate_event_against_parameters(event, &parameter_by_id)?;
        if event.kind != sonara_model::EventKind::Persistent {
            for asset_id in collect_event_asset_ids(event) {
                let asset = asset_by_id
                    .get(&asset_id)
                    .ok_or(BuildError::MissingAudioAsset)?;
                if asset.streaming == StreamingMode::Auto {
                    auto_assets_used_by_one_shot.insert(asset_id);
                }
            }
        }
        events.push((*event).clone());
    }

    for bus_id in &definition.buses {
        let bus = bus_by_id
            .get(bus_id)
            .ok_or(BuildError::MissingBusDefinition)?;
        buses.push((*bus).clone());
    }

    for snapshot_id in &definition.snapshots {
        let snapshot = snapshot_by_id
            .get(snapshot_id)
            .ok_or(BuildError::MissingSnapshotDefinition)?;
        snapshots.push((*snapshot).clone());
    }

    for graph_id in &definition.music_graphs {
        let graph = music_graph_by_id
            .get(graph_id)
            .ok_or(BuildError::MissingMusicGraphDefinition)?;
        let dependencies =
            validate_music_graph(graph, &clip_by_id, &resume_slot_by_id, &sync_domain_by_id)?;

        for clip_id in dependencies.clip_ids {
            push_unique(&mut clip_ids, clip_id);
        }
        for slot_id in dependencies.resume_slot_ids {
            push_unique(&mut resume_slot_ids, slot_id);
        }
        for sync_domain_id in dependencies.sync_domain_ids {
            push_unique(&mut sync_domain_ids, sync_domain_id);
        }

        music_graphs.push((*graph).clone());
    }

    for clip_id in &clip_ids {
        let clip = clip_by_id
            .get(clip_id)
            .ok_or(BuildError::MissingClipDefinition)?;
        clips.push((*clip).clone());
    }

    for slot_id in &resume_slot_ids {
        let slot = resume_slot_by_id
            .get(slot_id)
            .ok_or(BuildError::MissingResumeSlotDefinition)?;
        resume_slots.push((*slot).clone());
    }

    for sync_domain_id in &sync_domain_ids {
        let sync_domain = sync_domain_by_id
            .get(sync_domain_id)
            .ok_or(BuildError::MissingSyncDomainDefinition)?;
        sync_domains.push((*sync_domain).clone());
    }

    let mut bank = build_bank(definition.name.clone(), &events, &project.assets)?;
    bank.id = definition.id;
    bank.objects.buses = definition.buses.clone();
    bank.objects.snapshots = definition.snapshots.clone();
    bank.objects.music_graphs = definition.music_graphs.clone();
    bank.objects.clips = clip_ids.clone();
    bank.objects.resume_slots = resume_slot_ids.clone();
    bank.objects.sync_domains = sync_domain_ids.clone();

    for clip in &clips {
        let asset = asset_by_id
            .get(&clip.asset_id)
            .ok_or(BuildError::MissingAudioAsset)?;
        add_bank_asset_to_manifest(&mut bank, asset);
    }
    finalize_bank_manifest_media(&mut bank, &auto_assets_used_by_one_shot);

    Ok(CompiledBankPackage {
        bank,
        events,
        buses,
        snapshots,
        clips,
        resume_slots,
        sync_domains,
        music_graphs,
    })
}

/// 根据 authoring 项目里的 bank 定义编译并写出一份 compiled bank 文件。
///
/// 这条路径用于把 editor/authoring 层维护的 project 数据导出为 runtime 可直接加载的产物。
pub fn compile_bank_definition_to_file(
    definition: &BankDefinition,
    project: &AuthoringProject,
    output_path: impl AsRef<std::path::Path>,
) -> Result<CompiledBankPackage, ExportBankError> {
    let package = compile_bank_definition(definition, project)?;
    package.write_json_file(output_path)?;
    Ok(package)
}
