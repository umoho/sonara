// SPDX-License-Identifier: MPL-2.0

use camino::Utf8PathBuf;
use sonara_model::{
    AudioAsset, AuthoringProject, Bank, BankDefinition, Clip, EdgeTrigger, EntryPolicy,
    EnumParameter, Event, EventContentNode, EventContentRoot, EventId, EventKind, MemoryPolicy,
    MusicEdge, MusicGraph, MusicNode, MusicNodeId, NodeId, NodeRef, Parameter, ParameterId,
    ParameterScope, PlaybackTarget, ResumeSlot, SamplerNode, SequenceNode, SpatialMode,
    StreamingMode, SwitchCase, SwitchNode, SyncDomain, Track, TrackBinding, TrackGroup,
    TrackGroupMode, TrackRole,
};
use uuid::Uuid;

use super::*;

fn make_event(nodes: Vec<EventContentNode>, root: NodeId) -> Event {
    Event {
        id: EventId::new(),
        name: "player.footstep".into(),
        kind: EventKind::OneShot,
        root: EventContentRoot {
            root: NodeRef { id: root },
            nodes,
        },
        default_bus: None,
        spatial: SpatialMode::ThreeD,
        default_parameters: Vec::new(),
        voice_limit: None,
        steal_policy: None,
    }
}

fn make_asset(name: &str, streaming: StreamingMode) -> AudioAsset {
    let mut asset = AudioAsset::new(name, Utf8PathBuf::from(format!("audio/{name}.wav")));
    asset.streaming = streaming;
    asset
}

fn make_clip(name: &str, asset_id: Uuid) -> Clip {
    Clip::new(name, asset_id)
}

#[test]
fn validate_event_rejects_missing_root_node() {
    let sampler_id = NodeId::new();
    let event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id: Uuid::now_v7(),
        })],
        NodeId::new(),
    );

    assert!(matches!(
        validate_event(&event),
        Err(BuildError::MissingRootNode)
    ));
}

#[test]
fn validate_event_rejects_missing_child_reference() {
    let switch_id = NodeId::new();
    let missing_child = NodeId::new();
    let event = make_event(
        vec![EventContentNode::Switch(SwitchNode {
            id: switch_id,
            parameter_id: ParameterId::new(),
            cases: vec![SwitchCase {
                variant: "wood".into(),
                child: NodeRef { id: missing_child },
            }],
            default_case: None,
        })],
        switch_id,
    );

    assert!(matches!(
        validate_event(&event),
        Err(BuildError::MissingChildNode)
    ));
}

#[test]
fn build_bank_collects_resident_and_streaming_media() {
    let resident_asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    let streaming_asset = make_asset("music_forest", StreamingMode::Streaming);
    let sampler_a = NodeId::new();
    let sampler_b = NodeId::new();
    let root_id = NodeId::new();

    let event = make_event(
        vec![
            EventContentNode::Sequence(SequenceNode {
                id: root_id,
                children: vec![NodeRef { id: sampler_a }, NodeRef { id: sampler_b }],
            }),
            EventContentNode::Sampler(SamplerNode {
                id: sampler_a,
                asset_id: resident_asset.id,
            }),
            EventContentNode::Sampler(SamplerNode {
                id: sampler_b,
                asset_id: streaming_asset.id,
            }),
        ],
        root_id,
    );

    let bank = build_bank(
        "core",
        &[event],
        &[resident_asset.clone(), streaming_asset.clone()],
    )
    .expect("bank should build");

    assert_eq!(bank.name.as_str(), "core");
    assert_eq!(bank.objects.events.len(), 1);
    assert_eq!(bank.manifest.assets.len(), 2);
    assert_eq!(bank.manifest.resident_media, vec![resident_asset.id]);
    assert_eq!(bank.manifest.streaming_media, vec![streaming_asset.id]);
}

#[test]
fn build_bank_treats_auto_assets_for_persistent_events_as_streaming() {
    let auto_asset = make_asset("music_forest", StreamingMode::Auto);
    let sampler_id = NodeId::new();
    let mut event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id: auto_asset.id,
        })],
        sampler_id,
    );
    event.kind = EventKind::Persistent;

    let bank = build_bank("music", &[event], &[auto_asset.clone()]).expect("bank should build");

    assert!(bank.manifest.resident_media.is_empty());
    assert_eq!(bank.manifest.streaming_media, vec![auto_asset.id]);
}

#[test]
fn build_bank_keeps_auto_assets_resident_when_any_one_shot_uses_them() {
    let auto_asset = make_asset("shared_loop", StreamingMode::Auto);
    let persistent_sampler_id = NodeId::new();
    let one_shot_sampler_id = NodeId::new();

    let mut persistent_event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: persistent_sampler_id,
            asset_id: auto_asset.id,
        })],
        persistent_sampler_id,
    );
    persistent_event.kind = EventKind::Persistent;

    let one_shot_event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: one_shot_sampler_id,
            asset_id: auto_asset.id,
        })],
        one_shot_sampler_id,
    );

    let bank = build_bank(
        "mixed",
        &[persistent_event, one_shot_event],
        &[auto_asset.clone()],
    )
    .expect("bank should build");

    assert_eq!(bank.manifest.resident_media, vec![auto_asset.id]);
    assert!(bank.manifest.streaming_media.is_empty());
}

#[test]
fn build_bank_preserves_asset_import_settings_in_manifest() {
    let sampler_id = NodeId::new();
    let mut asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    asset.import_settings.normalize = true;
    asset.import_settings.target_sample_rate = Some(48_000);
    let event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id: asset.id,
        })],
        sampler_id,
    );

    let bank = build_bank("core", &[event], &[asset.clone()]).expect("bank should build");
    let manifest_asset = bank
        .manifest
        .assets
        .first()
        .expect("manifest asset should exist");

    assert_eq!(manifest_asset.id, asset.id);
    assert_eq!(manifest_asset.import_settings, asset.import_settings);
}

#[test]
fn build_bank_rejects_missing_asset() {
    let sampler_id = NodeId::new();
    let asset_id = Uuid::now_v7();
    let event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id,
        })],
        sampler_id,
    );

    assert!(matches!(
        build_bank("core", &[event], &[]),
        Err(BuildError::MissingAudioAsset)
    ));
}

#[test]
fn build_bank_from_definition_uses_project_event_selection() {
    let selected_asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    let ignored_asset = make_asset("ui_click", StreamingMode::Resident);
    let selected_sampler_id = NodeId::new();
    let ignored_sampler_id = NodeId::new();

    let selected_event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: selected_sampler_id,
            asset_id: selected_asset.id,
        })],
        selected_sampler_id,
    );
    let ignored_event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: ignored_sampler_id,
            asset_id: ignored_asset.id,
        })],
        ignored_sampler_id,
    );

    let mut project = AuthoringProject::new("demo");
    project.assets.push(selected_asset.clone());
    project.assets.push(ignored_asset);
    project.events.push(selected_event.clone());
    project.events.push(ignored_event);

    let mut definition = sonara_model::BankDefinition::new("core");
    definition.events.push(selected_event.id);

    let bank =
        build_bank_from_definition(&definition, &project).expect("bank should build from project");

    assert_eq!(bank.id, definition.id);
    assert_eq!(bank.objects.events, vec![selected_event.id]);
    assert_eq!(bank.manifest.assets.len(), 1);
    assert_eq!(bank.manifest.assets[0].id, selected_asset.id);
}

#[test]
fn build_bank_from_definition_rejects_missing_project_event() {
    let project = AuthoringProject::new("demo");
    let mut definition = sonara_model::BankDefinition::new("core");
    definition.events.push(EventId::new());

    assert!(matches!(
        build_bank_from_definition(&definition, &project),
        Err(BuildError::MissingEventDefinition)
    ));
}

#[test]
fn build_bank_from_definition_preserves_bus_and_snapshot_selection() {
    let mut project = AuthoringProject::new("demo");
    let bus = sonara_model::Bus::new("sfx");
    let snapshot = sonara_model::Snapshot {
        id: sonara_model::SnapshotId::new(),
        name: "combat".into(),
        fade_in_seconds: 0.2,
        fade_out_seconds: 0.4,
        targets: Vec::new(),
    };
    project.buses.push(bus.clone());
    project.snapshots.push(snapshot.clone());

    let mut definition = sonara_model::BankDefinition::new("core");
    definition.buses.push(bus.id);
    definition.snapshots.push(snapshot.id);

    let bank =
        build_bank_from_definition(&definition, &project).expect("bank should build from project");

    assert_eq!(bank.objects.buses, vec![bus.id]);
    assert_eq!(bank.objects.snapshots, vec![snapshot.id]);
}

#[test]
fn compile_bank_definition_returns_selected_object_definitions() {
    let selected_asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    let selected_sampler_id = NodeId::new();
    let selected_event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: selected_sampler_id,
            asset_id: selected_asset.id,
        })],
        selected_sampler_id,
    );
    let bus = sonara_model::Bus::new("sfx");
    let snapshot = sonara_model::Snapshot {
        id: sonara_model::SnapshotId::new(),
        name: "combat".into(),
        fade_in_seconds: 0.2,
        fade_out_seconds: 0.4,
        targets: vec![sonara_model::SnapshotTarget {
            bus_id: bus.id,
            target_volume: 0.8,
        }],
    };

    let mut project = AuthoringProject::new("demo");
    project.assets.push(selected_asset);
    project.events.push(selected_event.clone());
    project.buses.push(bus.clone());
    project.snapshots.push(snapshot.clone());

    let mut definition = sonara_model::BankDefinition::new("core");
    definition.events.push(selected_event.id);
    definition.buses.push(bus.id);
    definition.snapshots.push(snapshot.id);

    let package = compile_bank_definition(&definition, &project).expect("package should compile");

    assert_eq!(package.bank.id, definition.id);
    assert_eq!(package.events, vec![selected_event]);
    assert_eq!(package.buses, vec![bus]);
    assert_eq!(package.snapshots, vec![snapshot]);
}

#[test]
fn compile_bank_definition_rejects_missing_switch_parameter() {
    let asset = make_asset("music_explore", StreamingMode::Streaming);
    let switch_id = NodeId::new();
    let sampler_id = NodeId::new();
    let missing_parameter_id = ParameterId::new();
    let event = make_event(
        vec![
            EventContentNode::Switch(SwitchNode {
                id: switch_id,
                parameter_id: missing_parameter_id,
                cases: vec![SwitchCase {
                    variant: "explore".into(),
                    child: NodeRef { id: sampler_id },
                }],
                default_case: Some(NodeRef { id: sampler_id }),
            }),
            EventContentNode::Sampler(SamplerNode {
                id: sampler_id,
                asset_id: asset.id,
            }),
        ],
        switch_id,
    );
    let event_id = event.id;

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.events.push(event);

    let mut definition = BankDefinition::new("music");
    definition.events.push(event_id);

    assert!(matches!(
        compile_bank_definition(&definition, &project),
        Err(BuildError::MissingParameterDefinition)
    ));
}

#[test]
fn compile_bank_definition_rejects_unknown_switch_variant() {
    let asset = make_asset("music_explore", StreamingMode::Streaming);
    let switch_id = NodeId::new();
    let sampler_id = NodeId::new();
    let parameter_id = ParameterId::new();
    let event = make_event(
        vec![
            EventContentNode::Switch(SwitchNode {
                id: switch_id,
                parameter_id,
                cases: vec![SwitchCase {
                    variant: "combat".into(),
                    child: NodeRef { id: sampler_id },
                }],
                default_case: Some(NodeRef { id: sampler_id }),
            }),
            EventContentNode::Sampler(SamplerNode {
                id: sampler_id,
                asset_id: asset.id,
            }),
        ],
        switch_id,
    );
    let event_id = event.id;

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.parameters.push(Parameter::Enum(EnumParameter {
        id: parameter_id,
        name: "music_state".into(),
        scope: ParameterScope::Global,
        default_value: "explore".into(),
        variants: vec!["explore".into(), "stealth".into()],
    }));
    project.events.push(event);

    let mut definition = BankDefinition::new("music");
    definition.events.push(event_id);

    assert!(matches!(
        compile_bank_definition(&definition, &project),
        Err(BuildError::UnknownSwitchVariant)
    ));
}

#[test]
fn compiled_bank_package_json_round_trip_preserves_bank_name() {
    let package = CompiledBankPackage {
        bank: Bank::new("core"),
        events: Vec::new(),
        buses: Vec::new(),
        snapshots: Vec::new(),
        clips: Vec::new(),
        resume_slots: Vec::new(),
        sync_domains: Vec::new(),
        music_graphs: Vec::new(),
    };

    let json = package
        .to_json_string_pretty()
        .expect("compiled package should serialize");
    let decoded = CompiledBankPackage::from_json_str(&json)
        .expect("compiled package should deserialize from JSON");

    assert_eq!(decoded.bank.name, "core");
}

#[test]
fn compile_bank_definition_collects_selected_music_graph_dependencies() {
    let asset = make_asset("boss_theme", StreamingMode::Auto);
    let mut clip = make_clip("boss_loop", asset.id);
    let sync_domain = SyncDomain::new("boss_sync");
    clip.sync_domain = Some(sync_domain.id);
    let resume_slot = ResumeSlot::new("boss_memory");
    let state_id = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("boss_flow");
    graph.initial_node = Some(state_id);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: Some(resume_slot.id),
        memory_policy: MemoryPolicy {
            ttl_seconds: Some(12.0),
            reset_to: EntryPolicy::ClipStart,
        },
        default_entry: EntryPolicy::Resume,
        externally_targetable: true,
        completion_source: None,
    });
    graph.edges.push(MusicEdge {
        from: state_id,
        to: state_id,
        requested_target: None,
        trigger: EdgeTrigger::NextMatchingCue {
            tag: "loop_out".into(),
        },
        destination: EntryPolicy::SameSyncPosition,
    });

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset.clone());
    project.clips.push(clip.clone());
    project.resume_slots.push(resume_slot.clone());
    project.sync_domains.push(sync_domain.clone());
    project.music_graphs.push(graph.clone());

    let mut definition = BankDefinition::new("music");
    definition.music_graphs.push(graph.id);

    let package =
        compile_bank_definition(&definition, &project).expect("music graph should compile");

    assert_eq!(package.bank.objects.music_graphs, vec![graph.id]);
    assert_eq!(package.bank.objects.clips, vec![clip.id]);
    assert_eq!(package.bank.objects.resume_slots, vec![resume_slot.id]);
    assert_eq!(package.bank.objects.sync_domains, vec![sync_domain.id]);
    assert_eq!(package.clips, vec![clip]);
    assert_eq!(package.resume_slots, vec![resume_slot]);
    assert_eq!(package.sync_domains, vec![sync_domain]);
    assert_eq!(package.music_graphs, vec![graph]);
    assert_eq!(package.bank.manifest.assets.len(), 1);
    assert_eq!(package.bank.manifest.assets[0].id, asset.id);
    assert_eq!(package.bank.manifest.streaming_media, vec![asset.id]);
}

#[test]
fn compile_bank_definition_rejects_music_graph_missing_clip() {
    let resume_slot = ResumeSlot::new("boss_memory");
    let state_id = MusicNodeId::new();
    let missing_clip_id = sonara_model::ClipId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("boss_flow");
    graph.initial_node = Some(state_id);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: missing_clip_id,
            },
        }],
        memory_slot: Some(resume_slot.id),
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });

    let mut project = AuthoringProject::new("demo");
    project.resume_slots.push(resume_slot);
    project.music_graphs.push(graph.clone());

    let mut definition = BankDefinition::new("music");
    definition.music_graphs.push(graph.id);

    assert!(matches!(
        compile_bank_definition(&definition, &project),
        Err(BuildError::MissingClipDefinition)
    ));
}

#[test]
fn compile_bank_definition_rejects_music_graph_binding_missing_track() {
    let asset = make_asset("boss_theme", StreamingMode::Auto);
    let clip = make_clip("boss_loop", asset.id);
    let state_id = MusicNodeId::new();
    let missing_track_id = Track::new("main", TrackRole::Main).id;
    let mut graph = MusicGraph::new("boss_flow");
    graph.initial_node = Some(state_id);
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: missing_track_id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.clips.push(clip);
    project.music_graphs.push(graph.clone());

    let mut definition = BankDefinition::new("music");
    definition.music_graphs.push(graph.id);

    assert!(matches!(
        compile_bank_definition(&definition, &project),
        Err(BuildError::MissingTrackDefinition)
    ));
}

#[test]
fn compile_bank_definition_rejects_music_graph_track_missing_group() {
    let asset = make_asset("boss_theme", StreamingMode::Auto);
    let clip = make_clip("boss_loop", asset.id);
    let node_id = MusicNodeId::new();
    let mut track = Track::new("main", TrackRole::Main);
    track.group = Some(sonara_model::TrackGroupId::new());
    let mut graph = MusicGraph::new("boss_flow");
    graph.initial_node = Some(node_id);
    graph.tracks.push(track.clone());
    graph.nodes.push(MusicNode {
        id: node_id,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.clips.push(clip);
    project.music_graphs.push(graph.clone());

    let mut definition = BankDefinition::new("music");
    definition.music_graphs.push(graph.id);

    assert!(matches!(
        compile_bank_definition(&definition, &project),
        Err(BuildError::MissingTrackGroupDefinition)
    ));
}

#[test]
fn compile_bank_definition_preserves_music_track_groups() {
    let asset = make_asset("boss_theme", StreamingMode::Auto);
    let clip = make_clip("boss_loop", asset.id);
    let node_id = MusicNodeId::new();
    let group = TrackGroup::new("day_style", TrackGroupMode::Exclusive);
    let mut track = Track::new("main", TrackRole::Main);
    track.group = Some(group.id);
    let mut graph = MusicGraph::new("boss_flow");
    graph.initial_node = Some(node_id);
    graph.groups.push(group.clone());
    graph.tracks.push(track.clone());
    graph.nodes.push(MusicNode {
        id: node_id,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.clips.push(clip);
    project.music_graphs.push(graph.clone());

    let mut definition = BankDefinition::new("music");
    definition.music_graphs.push(graph.id);

    let package = compile_bank_definition(&definition, &project)
        .expect("bank should compile with track groups");
    let compiled_graph = package
        .music_graphs
        .first()
        .expect("compiled package should contain the graph");

    assert_eq!(compiled_graph.groups, vec![group]);
    assert_eq!(
        compiled_graph.tracks[0].group,
        Some(compiled_graph.groups[0].id)
    );
}

#[test]
fn compiled_bank_package_keeps_object_lists_in_sync_with_loaded_definitions() {
    let asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    let sampler_id = NodeId::new();
    let event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id: asset.id,
        })],
        sampler_id,
    );
    let bus = sonara_model::Bus::new("sfx");
    let snapshot = sonara_model::Snapshot {
        id: sonara_model::SnapshotId::new(),
        name: "combat".into(),
        fade_in_seconds: 0.2,
        fade_out_seconds: 0.4,
        targets: vec![sonara_model::SnapshotTarget {
            bus_id: bus.id,
            target_volume: 0.8,
        }],
    };

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.events.push(event.clone());
    project.buses.push(bus.clone());
    project.snapshots.push(snapshot.clone());

    let mut definition = sonara_model::BankDefinition::new("core");
    definition.events.push(event.id);
    definition.buses.push(bus.id);
    definition.snapshots.push(snapshot.id);

    let package = compile_bank_definition(&definition, &project).expect("package should compile");

    assert_eq!(package.bank().objects.events, vec![event.id]);
    assert_eq!(package.bank().objects.buses, vec![bus.id]);
    assert_eq!(package.bank().objects.snapshots, vec![snapshot.id]);
    assert_eq!(
        package
            .events()
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>(),
        vec![event.id]
    );
    assert_eq!(
        package.buses().iter().map(|bus| bus.id).collect::<Vec<_>>(),
        vec![bus.id]
    );
    assert_eq!(
        package
            .snapshots()
            .iter()
            .map(|snapshot| snapshot.id)
            .collect::<Vec<_>>(),
        vec![snapshot.id]
    );
}

#[test]
fn compiled_bank_package_manifest_only_contains_assets_referenced_by_loaded_events() {
    let selected_asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    let ignored_asset = make_asset("ui_click", StreamingMode::Resident);
    let sampler_id = NodeId::new();
    let event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id: selected_asset.id,
        })],
        sampler_id,
    );

    let mut project = AuthoringProject::new("demo");
    project.assets.push(selected_asset.clone());
    project.assets.push(ignored_asset.clone());
    project.events.push(event.clone());

    let mut definition = sonara_model::BankDefinition::new("core");
    definition.events.push(event.id);

    let package = compile_bank_definition(&definition, &project).expect("package should compile");

    assert_eq!(package.bank().manifest.assets.len(), 1);
    assert_eq!(package.bank().manifest.assets[0].id, selected_asset.id);
    assert!(
        !package
            .bank()
            .manifest
            .assets
            .iter()
            .any(|asset| asset.id == ignored_asset.id)
    );
}

#[test]
fn compile_bank_definition_to_file_writes_compiled_bank_json() {
    let asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    let sampler_id = NodeId::new();
    let event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id: asset.id,
        })],
        sampler_id,
    );

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.events.push(event.clone());

    let mut definition = sonara_model::BankDefinition::new("core");
    definition.events.push(event.id);

    let output_path =
        std::env::temp_dir().join(format!("sonara-compiled-bank-{}.json", Uuid::now_v7()));
    let package = compile_bank_definition_to_file(&definition, &project, &output_path)
        .expect("compiled bank export should succeed");
    let decoded = CompiledBankPackage::read_json_file(&output_path)
        .expect("exported compiled bank file should be readable");

    assert_eq!(decoded.bank.id, package.bank.id);
    assert_eq!(decoded.bank.name, "core");

    std::fs::remove_file(output_path).expect("temp compiled bank file should be removed");
}

#[test]
fn compile_project_bank_uses_named_bank_definition() {
    let asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    let sampler_id = NodeId::new();
    let event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id: asset.id,
        })],
        sampler_id,
    );

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.events.push(event.clone());

    let mut definition = sonara_model::BankDefinition::new("core");
    definition.events.push(event.id);
    project.banks.push(definition.clone());

    let package =
        compile_project_bank(&project, "core").expect("named project bank should compile");

    assert_eq!(package.bank.id, definition.id);
    assert_eq!(package.events, vec![event]);
}

#[test]
fn compile_project_bank_file_to_file_reads_project_and_writes_output() {
    let asset = make_asset("footstep_wood_01", StreamingMode::Resident);
    let sampler_id = NodeId::new();
    let event = make_event(
        vec![EventContentNode::Sampler(SamplerNode {
            id: sampler_id,
            asset_id: asset.id,
        })],
        sampler_id,
    );

    let mut project = AuthoringProject::new("demo");
    project.assets.push(asset);
    project.events.push(event.clone());

    let mut definition = sonara_model::BankDefinition::new("core");
    definition.events.push(event.id);
    project.banks.push(definition);

    let project_path = std::env::temp_dir().join(format!("sonara-project-{}.json", Uuid::now_v7()));
    let output_path =
        std::env::temp_dir().join(format!("sonara-project-bank-{}.json", Uuid::now_v7()));

    project
        .write_json_file(&project_path)
        .expect("temp project file should be written");

    let package = compile_project_bank_file_to_file(&project_path, "core", &output_path)
        .expect("project file export should succeed");
    let decoded = CompiledBankPackage::read_json_file(&output_path)
        .expect("exported compiled bank file should be readable");

    assert_eq!(decoded.bank.id, package.bank.id);
    assert_eq!(decoded.events, package.events);

    std::fs::remove_file(project_path).expect("temp project file should be removed");
    std::fs::remove_file(output_path).expect("temp compiled bank file should be removed");
}
