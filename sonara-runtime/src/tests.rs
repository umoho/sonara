use smol_str::SmolStr;
use sonara_model::{
    Bank, Bus, BusEffectSlot, BusId, Clip, CuePoint, EdgeTrigger, EntryPolicy, Event,
    EventContentNode, EventContentRoot, EventId, EventKind, MemoryPolicy, MusicEdge, MusicGraph,
    MusicNode, MusicNodeId, NodeId, NodeRef, ParameterId, ParameterValue, PlaybackTarget,
    ResumeSlot, SamplerNode, SequenceNode, Snapshot, SnapshotId, SnapshotTarget, SpatialMode,
    SwitchCase, SwitchNode, SyncDomain, TimeRange, Track, TrackBinding, TrackGroup, TrackGroupMode,
    TrackRole,
};
use uuid::Uuid;

use super::*;

fn make_sampler(asset_id: Uuid) -> (NodeId, EventContentNode) {
    let id = NodeId::new();
    (id, EventContentNode::Sampler(SamplerNode { id, asset_id }))
}

fn make_event(id: EventId, root: NodeId, nodes: Vec<EventContentNode>) -> Event {
    Event {
        id,
        name: SmolStr::new("player.footstep"),
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

#[test]
fn play_creates_an_active_instance_with_plan() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let (sampler_id, sampler) = make_sampler(asset_id);
    let event = make_event(event_id, sampler_id, vec![sampler]);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let instance_id = runtime.play(event_id).expect("event should play");

    assert_eq!(
        runtime.active_plan(instance_id),
        Some(&PlaybackPlan {
            event_id,
            emitter_id: None,
            asset_ids: vec![asset_id],
        })
    );
}

#[test]
fn plan_event_resolves_switch_from_global_param() {
    let event_id = EventId::new();
    let surface_id = ParameterId::new();
    let switch_id = NodeId::new();
    let wood_asset = Uuid::now_v7();
    let stone_asset = Uuid::now_v7();
    let (wood_node_id, wood_sampler) = make_sampler(wood_asset);
    let (stone_node_id, stone_sampler) = make_sampler(stone_asset);

    let event = make_event(
        event_id,
        switch_id,
        vec![
            EventContentNode::Switch(SwitchNode {
                id: switch_id,
                parameter_id: surface_id,
                cases: vec![
                    SwitchCase {
                        variant: "wood".into(),
                        child: NodeRef { id: wood_node_id },
                    },
                    SwitchCase {
                        variant: "stone".into(),
                        child: NodeRef { id: stone_node_id },
                    },
                ],
                default_case: Some(NodeRef { id: wood_node_id }),
            }),
            wood_sampler,
            stone_sampler,
        ],
    );

    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event])
        .expect("bank should load");
    runtime
        .set_global_param(surface_id, ParameterValue::Enum("stone".into()))
        .expect("param should set");

    let plan = runtime.plan_event(event_id).expect("plan should resolve");

    assert_eq!(plan.asset_ids, vec![stone_asset]);
}

#[test]
fn plan_event_on_prefers_emitter_param_over_global_param() {
    let event_id = EventId::new();
    let surface_id = ParameterId::new();
    let switch_id = NodeId::new();
    let wood_asset = Uuid::now_v7();
    let stone_asset = Uuid::now_v7();
    let (wood_node_id, wood_sampler) = make_sampler(wood_asset);
    let (stone_node_id, stone_sampler) = make_sampler(stone_asset);

    let event = make_event(
        event_id,
        switch_id,
        vec![
            EventContentNode::Switch(SwitchNode {
                id: switch_id,
                parameter_id: surface_id,
                cases: vec![
                    SwitchCase {
                        variant: "wood".into(),
                        child: NodeRef { id: wood_node_id },
                    },
                    SwitchCase {
                        variant: "stone".into(),
                        child: NodeRef { id: stone_node_id },
                    },
                ],
                default_case: Some(NodeRef { id: wood_node_id }),
            }),
            wood_sampler,
            stone_sampler,
        ],
    );

    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event])
        .expect("bank should load");
    let emitter_id = runtime.create_emitter();
    runtime
        .set_global_param(surface_id, ParameterValue::Enum("wood".into()))
        .expect("param should set");
    runtime
        .set_emitter_param(emitter_id, surface_id, ParameterValue::Enum("stone".into()))
        .expect("emitter param should set");

    let plan = runtime
        .plan_event_on(emitter_id, event_id)
        .expect("plan should resolve");

    assert_eq!(plan.asset_ids, vec![stone_asset]);
    assert_eq!(plan.emitter_id, Some(emitter_id));
}

#[test]
fn plan_event_resolves_sequence_children_in_order() {
    let event_id = EventId::new();
    let root_id = NodeId::new();
    let asset_a = Uuid::now_v7();
    let asset_b = Uuid::now_v7();
    let (node_a, sampler_a) = make_sampler(asset_a);
    let (node_b, sampler_b) = make_sampler(asset_b);

    let event = make_event(
        event_id,
        root_id,
        vec![
            EventContentNode::Sequence(SequenceNode {
                id: root_id,
                children: vec![NodeRef { id: node_a }, NodeRef { id: node_b }],
            }),
            sampler_a,
            sampler_b,
        ],
    );

    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let plan = runtime.plan_event(event_id).expect("plan should resolve");

    assert_eq!(plan.asset_ids, vec![asset_a, asset_b]);
}

#[test]
fn audio_command_buffer_applies_requests_in_order() {
    let mut buffer = AudioCommandBuffer::new();
    buffer.push(1);
    buffer.push(2);

    let results = buffer
        .apply(|value| Ok::<_, ()>(value * 10))
        .expect("apply should succeed");

    assert_eq!(results, vec![10, 20]);
    assert!(buffer.is_empty());
}

#[test]
fn audio_command_buffer_isolates_per_request_failures() {
    let mut buffer = AudioCommandBuffer::new();
    buffer.push(1);
    buffer.push(2);
    buffer.push(3);

    let outcomes = buffer.apply_isolated(|value| {
        if *value == 2 {
            Err("boom")
        } else {
            Ok(value * 10)
        }
    });

    assert_eq!(outcomes.len(), 3);
    assert!(matches!(outcomes[0].result, Ok(10)));
    assert!(matches!(outcomes[1].result, Err("boom")));
    assert!(matches!(outcomes[2].result, Ok(30)));
    assert!(buffer.is_empty());
}

#[test]
fn stop_request_removes_active_instance() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let (sampler_id, sampler) = make_sampler(asset_id);
    let event = make_event(event_id, sampler_id, vec![sampler]);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let instance_id = runtime.play(event_id).expect("event should play");
    let result = runtime
        .apply_request(&RuntimeRequest::stop(instance_id, Fade::IMMEDIATE))
        .expect("stop should succeed");

    assert_eq!(result, RuntimeRequestResult::Stopped { instance_id });
    assert_eq!(runtime.active_plan(instance_id), None);
}

#[test]
fn instance_state_reports_playing_then_stopped() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let (sampler_id, sampler) = make_sampler(asset_id);
    let event = make_event(event_id, sampler_id, vec![sampler]);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let instance_id = runtime.play(event_id).expect("event should play");
    assert_eq!(
        runtime.instance_state(instance_id),
        EventInstanceState::Playing
    );

    runtime
        .stop(instance_id, Fade::IMMEDIATE)
        .expect("stop should succeed");
    assert_eq!(
        runtime.instance_state(instance_id),
        EventInstanceState::Stopped
    );
}

#[test]
fn load_bank_keeps_only_compiled_objects_in_runtime_state() {
    let event_id = EventId::new();
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);
    let bank_id = bank.id;

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, Vec::new())
        .expect("bank should load");

    let objects = runtime
        .loaded_bank_objects(bank_id)
        .expect("loaded bank objects should exist");

    assert_eq!(objects.events, vec![event_id]);
}

#[test]
fn load_bank_with_definitions_registers_music_foundation_objects() {
    let asset_id = Uuid::now_v7();
    let clip = Clip::new("explore_main", asset_id);
    let resume_slot = ResumeSlot::new("explore_memory");
    let sync_domain = SyncDomain::new("day_night");
    let state_id = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("world_music");
    graph.initial_node = Some(state_id);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "explore".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: Some(resume_slot.id),
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.resume_slots.push(resume_slot.id);
    bank.objects.sync_domains.push(sync_domain.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip.clone()],
            vec![resume_slot.clone()],
            vec![sync_domain.clone()],
            vec![graph.clone()],
        )
        .expect("bank should load with music definitions");

    assert_eq!(runtime.clip(clip.id), Some(&clip));
    assert_eq!(runtime.resume_slot(resume_slot.id), Some(&resume_slot));
    assert_eq!(runtime.sync_domain(sync_domain.id), Some(&sync_domain));
    assert_eq!(runtime.music_graph(graph.id), Some(&graph));
}

#[test]
fn unload_bank_removes_music_foundation_objects() {
    let asset_id = Uuid::now_v7();
    let clip = Clip::new("combat_main", asset_id);
    let resume_slot = ResumeSlot::new("combat_memory");
    let state_id = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("combat_music");
    graph.initial_node = Some(state_id);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "combat".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: Some(resume_slot.id),
        memory_policy: MemoryPolicy {
            ttl_seconds: Some(30.0),
            reset_to: EntryPolicy::ClipStart,
        },
        default_entry: EntryPolicy::Resume,
        externally_targetable: true,
        completion_source: None,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.resume_slots.push(resume_slot.id);
    bank.objects.music_graphs.push(graph.id);
    let bank_id = bank.id;

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip.clone()],
            vec![resume_slot.clone()],
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music definitions");

    runtime.unload_bank(bank_id).expect("bank should unload");

    assert_eq!(runtime.clip(clip.id), None);
    assert_eq!(runtime.resume_slot(resume_slot.id), None);
    assert_eq!(runtime.music_graph(graph.id), None);
}

#[test]
fn play_music_graph_uses_declared_initial_node() {
    let asset_id = Uuid::now_v7();
    let clip = Clip::new("explore_main", asset_id);
    let explore_state = MusicNodeId::new();
    let combat_state = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("world_music");
    graph.initial_node = Some(combat_state);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: explore_state,
        name: "explore".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });
    graph.nodes.push(MusicNode {
        id: combat_state,
        name: "combat".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    let status = runtime
        .music_status(session_id)
        .expect("music status should resolve");

    assert_eq!(status.active_node, combat_state);
    assert_eq!(status.desired_target_node, combat_state);
    assert_eq!(status.phase, MusicPhase::Stable);
}

#[test]
fn request_music_node_tracks_pending_transition_until_bridge_completes() {
    let asset_id = Uuid::now_v7();
    let clip = Clip::new("preheat_loop", asset_id);
    let bridge_clip = Clip::new("transition", Uuid::now_v7());
    let boss_clip = Clip::new("boss_loop", Uuid::now_v7());
    let preheat_state = MusicNodeId::new();
    let bridge_state = MusicNodeId::new();
    let boss_state = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let bridge_track = Track::new("music_bridge", TrackRole::Bridge);
    let mut graph = MusicGraph::new("boss_music");
    graph.initial_node = Some(preheat_state);
    graph.tracks.push(main_track.clone());
    graph.tracks.push(bridge_track.clone());
    graph.nodes.push(MusicNode {
        id: preheat_state,
        name: "preheat".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });
    graph.nodes.push(MusicNode {
        id: bridge_state,
        name: "bridge".into(),
        bindings: vec![TrackBinding {
            track_id: bridge_track.id,
            target: PlaybackTarget::Clip {
                clip_id: bridge_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: false,
        completion_source: Some(bridge_track.id),
    });
    graph.nodes.push(MusicNode {
        id: boss_state,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: boss_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });
    graph.edges.push(MusicEdge {
        from: preheat_state,
        to: bridge_state,
        requested_target: Some(boss_state),
        trigger: EdgeTrigger::NextMatchingCue {
            tag: "battle_ready".into(),
        },
        destination: EntryPolicy::ClipStart,
    });
    graph.edges.push(MusicEdge {
        from: bridge_state,
        to: boss_state,
        requested_target: Some(boss_state),
        trigger: EdgeTrigger::OnComplete,
        destination: EntryPolicy::EntryCue {
            tag: "boss_in".into(),
        },
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.clips.push(bridge_clip.id);
    bank.objects.clips.push(boss_clip.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip.clone(), bridge_clip.clone(), boss_clip.clone()],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    runtime
        .request_music_node(session_id, boss_state)
        .expect("node request should succeed");

    let waiting_status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(waiting_status.active_node, preheat_state);
    assert_eq!(waiting_status.desired_target_node, boss_state);
    assert_eq!(waiting_status.phase, MusicPhase::WaitingExitCue);
    assert_eq!(waiting_status.current_track_id, Some(main_track.id));

    runtime
        .complete_music_exit(session_id)
        .expect("exit cue completion should succeed");
    let bridge_status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(bridge_status.phase, MusicPhase::WaitingNodeCompletion);
    assert_eq!(bridge_status.current_track_id, Some(bridge_track.id));

    let bridge_playback = runtime
        .resolve_music_playback(session_id, 0.0)
        .expect("bridge playback should resolve");
    assert_eq!(bridge_playback.clip_id, bridge_clip.id);
    assert_eq!(bridge_playback.track_id, Some(bridge_track.id));

    runtime
        .complete_music_node_completion(session_id)
        .expect("bridge completion should succeed");
    let stable_status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(stable_status.active_node, boss_state);
    assert_eq!(stable_status.desired_target_node, boss_state);
    assert_eq!(stable_status.phase, MusicPhase::Stable);
    assert!(stable_status.pending_transition.is_none());
}

#[test]
fn initial_node_can_auto_advance_on_complete() {
    let intro_clip = Clip::new("intro", Uuid::now_v7());
    let warmup_clip = Clip::new("warmup", Uuid::now_v7());
    let intro_node = MusicNodeId::new();
    let warmup_node = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("interactive_music");
    graph.initial_node = Some(intro_node);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: intro_node,
        name: "intro".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: intro_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: false,
        completion_source: Some(main_track.id),
    });
    graph.nodes.push(MusicNode {
        id: warmup_node,
        name: "warmup".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: warmup_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: Some(main_track.id),
    });
    graph.edges.push(MusicEdge {
        from: intro_node,
        to: warmup_node,
        requested_target: None,
        trigger: EdgeTrigger::OnComplete,
        destination: EntryPolicy::ClipStart,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.extend([intro_clip.id, warmup_clip.id]);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![intro_clip.clone(), warmup_clip.clone()],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    let status = runtime
        .music_status(session_id)
        .expect("music status should resolve");

    assert_eq!(status.active_node, intro_node);
    assert_eq!(status.desired_target_node, intro_node);
    assert_eq!(status.phase, MusicPhase::WaitingNodeCompletion);

    runtime
        .complete_music_node_completion(session_id)
        .expect("intro completion should advance to warmup");
    let warmup_status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(warmup_status.active_node, warmup_node);
    assert_eq!(warmup_status.phase, MusicPhase::Stable);
}

#[test]
fn requested_on_complete_edge_retargets_current_looping_node() {
    let warmup_clip = Clip::new("warmup", Uuid::now_v7());
    let transition_clip = Clip::new("transition", Uuid::now_v7());
    let climax_clip = Clip::new("climax", Uuid::now_v7());
    let warmup_node = MusicNodeId::new();
    let transition_node = MusicNodeId::new();
    let climax_node = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("interactive_music");
    graph.initial_node = Some(warmup_node);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: warmup_node,
        name: "warmup".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: warmup_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: Some(main_track.id),
    });
    graph.nodes.push(MusicNode {
        id: transition_node,
        name: "transition".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: transition_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: false,
        completion_source: Some(main_track.id),
    });
    graph.nodes.push(MusicNode {
        id: climax_node,
        name: "climax".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: climax_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: Some(main_track.id),
    });
    graph.edges.push(MusicEdge {
        from: warmup_node,
        to: warmup_node,
        requested_target: None,
        trigger: EdgeTrigger::OnComplete,
        destination: EntryPolicy::ClipStart,
    });
    graph.edges.push(MusicEdge {
        from: warmup_node,
        to: transition_node,
        requested_target: Some(climax_node),
        trigger: EdgeTrigger::OnComplete,
        destination: EntryPolicy::ClipStart,
    });
    graph.edges.push(MusicEdge {
        from: transition_node,
        to: climax_node,
        requested_target: Some(climax_node),
        trigger: EdgeTrigger::OnComplete,
        destination: EntryPolicy::ClipStart,
    });

    let mut bank = Bank::new("core");
    bank.objects
        .clips
        .extend([warmup_clip.id, transition_clip.id, climax_clip.id]);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![
                warmup_clip.clone(),
                transition_clip.clone(),
                climax_clip.clone(),
            ],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    let initial_status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(initial_status.active_node, warmup_node);
    assert_eq!(initial_status.phase, MusicPhase::WaitingNodeCompletion);

    runtime
        .request_music_node(session_id, climax_node)
        .expect("climax request should replace warmup self-loop");
    let waiting_status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(waiting_status.active_node, warmup_node);
    assert_eq!(waiting_status.desired_target_node, climax_node);
    assert_eq!(waiting_status.phase, MusicPhase::WaitingNodeCompletion);
    assert_eq!(
        waiting_status
            .pending_transition
            .as_ref()
            .expect("pending transition should exist")
            .to_node,
        transition_node
    );

    runtime
        .complete_music_node_completion(session_id)
        .expect("warmup completion should enter transition");
    let transition_status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(transition_status.active_node, transition_node);
    assert_eq!(transition_status.phase, MusicPhase::WaitingNodeCompletion);

    runtime
        .complete_music_node_completion(session_id)
        .expect("transition completion should enter climax");
    let climax_status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(climax_status.active_node, climax_node);
    assert_eq!(climax_status.phase, MusicPhase::Stable);
}

#[test]
fn resolve_music_playback_uses_saved_resume_position_when_memory_is_fresh() {
    let clip = Clip::new("explore_main", Uuid::now_v7());
    let resume_slot = ResumeSlot::new("explore_memory");
    let state_id = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("world_music");
    graph.initial_node = Some(state_id);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "explore".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: Some(resume_slot.id),
        memory_policy: MemoryPolicy {
            ttl_seconds: Some(30.0),
            reset_to: EntryPolicy::ClipStart,
        },
        default_entry: EntryPolicy::Resume,
        externally_targetable: true,
        completion_source: None,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.resume_slots.push(resume_slot.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip.clone()],
            vec![resume_slot.clone()],
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    assert!(
        runtime
            .save_music_session_resume_position(session_id, 12.5, 10.0)
            .expect("resume save should succeed")
    );

    let resolved = runtime
        .resolve_music_playback(session_id, 20.0)
        .expect("music playback should resolve");

    assert_eq!(resolved.clip_id, clip.id);
    assert_eq!(resolved.entry_offset_seconds, 12.5);
    assert_eq!(
        runtime
            .resume_memory(resume_slot.id)
            .unwrap()
            .position_seconds,
        12.5
    );
}

#[test]
fn resolve_music_playback_falls_back_to_clip_start_after_resume_ttl_expires() {
    let clip = Clip::new("explore_main", Uuid::now_v7());
    let resume_slot = ResumeSlot::new("explore_memory");
    let state_id = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("world_music");
    graph.initial_node = Some(state_id);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "explore".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: Some(resume_slot.id),
        memory_policy: MemoryPolicy {
            ttl_seconds: Some(5.0),
            reset_to: EntryPolicy::ClipStart,
        },
        default_entry: EntryPolicy::Resume,
        externally_targetable: true,
        completion_source: None,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.resume_slots.push(resume_slot.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip],
            vec![resume_slot],
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    runtime
        .save_music_session_resume_position(session_id, 9.0, 10.0)
        .expect("resume save should succeed");

    let resolved = runtime
        .resolve_music_playback(session_id, 20.0)
        .expect("music playback should resolve");

    assert_eq!(resolved.entry_offset_seconds, 0.0);
}

#[test]
fn immediate_music_transition_uses_destination_resume_entry() {
    let explore_clip = Clip::new("explore_main", Uuid::now_v7());
    let combat_clip = Clip::new("combat_main", Uuid::now_v7());
    let combat_memory = ResumeSlot::new("combat_memory");
    let explore_state = MusicNodeId::new();
    let combat_state = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("world_music");
    graph.initial_node = Some(explore_state);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: explore_state,
        name: "explore".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: explore_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });
    graph.nodes.push(MusicNode {
        id: combat_state,
        name: "combat".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: combat_clip.id,
            },
        }],
        memory_slot: Some(combat_memory.id),
        memory_policy: MemoryPolicy {
            ttl_seconds: Some(30.0),
            reset_to: EntryPolicy::ClipStart,
        },
        default_entry: EntryPolicy::Resume,
        externally_targetable: true,
        completion_source: None,
    });
    graph.edges.push(MusicEdge {
        from: explore_state,
        to: combat_state,
        requested_target: None,
        trigger: EdgeTrigger::Immediate,
        destination: EntryPolicy::Resume,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.extend([explore_clip.id, combat_clip.id]);
    bank.objects.resume_slots.push(combat_memory.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![explore_clip, combat_clip.clone()],
            vec![combat_memory.clone()],
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    runtime.resume_memories.insert(
        combat_memory.id,
        ResumeMemoryEntry {
            position_seconds: 18.0,
            saved_at_seconds: 10.0,
        },
    );

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    runtime
        .request_music_node(session_id, combat_state)
        .expect("music node request should succeed");

    let resolved = runtime
        .resolve_music_playback(session_id, 20.0)
        .expect("music playback should resolve");

    assert_eq!(resolved.clip_id, combat_clip.id);
    assert_eq!(resolved.entry_offset_seconds, 18.0);
}

#[test]
fn resolve_music_playback_uses_entry_cue_offset() {
    let mut clip = Clip::new("boss_loop", Uuid::now_v7());
    let mut first = CuePoint::new("boss_intro", 4.0);
    first.tags.push("boss_in".into());
    let mut second = CuePoint::new("boss_intro_2", 9.0);
    second.tags.push("boss_in".into());
    clip.cues = vec![second, first];

    let state_id = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("boss_music");
    graph.initial_node = Some(state_id);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::EntryCue {
            tag: "boss_in".into(),
        },
        externally_targetable: true,
        completion_source: None,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip.clone()],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    let resolved = runtime
        .resolve_music_playback(session_id, 0.0)
        .expect("music playback should resolve");

    assert_eq!(resolved.clip_id, clip.id);
    assert_eq!(resolved.entry_offset_seconds, 4.0);
}

#[test]
fn explicit_main_track_binding_drives_playback_target() {
    let main_clip = Clip::new("explicit_main", Uuid::now_v7());
    let state_id = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("boss_music");
    graph.initial_node = Some(state_id);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: state_id,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: main_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(main_clip.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![main_clip.clone()],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with explicit main track");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    let status = runtime
        .music_status(session_id)
        .expect("music status should resolve");
    let resolved = runtime
        .resolve_music_playback(session_id, 0.0)
        .expect("music playback should resolve");

    assert_eq!(
        status.current_target,
        Some(PlaybackTarget::Clip {
            clip_id: main_clip.id,
        })
    );
    assert_eq!(resolved.clip_id, main_clip.id);
}

#[test]
fn track_group_state_defaults_to_active_and_can_be_toggled() {
    let clip = Clip::new("layered", Uuid::now_v7());
    let node_id = MusicNodeId::new();
    let mut grouped_track = Track::new("music_layer", TrackRole::Layer);
    let group = TrackGroup::new("combat_layer", TrackGroupMode::Additive);
    grouped_track.group = Some(group.id);

    let mut graph = MusicGraph::new("layered_music");
    graph.initial_node = Some(node_id);
    graph.groups.push(group.clone());
    graph.tracks.push(grouped_track.clone());
    graph.nodes.push(MusicNode {
        id: node_id,
        name: "layered".into(),
        bindings: vec![TrackBinding {
            track_id: grouped_track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: Some(grouped_track.id),
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with track group");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    assert_eq!(
        runtime
            .music_track_group_state(session_id, group.id)
            .expect("group state should resolve"),
        TrackGroupState { active: true }
    );

    runtime
        .set_music_track_group_active(session_id, group.id, false)
        .expect("track group should toggle");
    assert_eq!(
        runtime
            .music_track_group_state(session_id, group.id)
            .expect("group state should resolve"),
        TrackGroupState { active: false }
    );
}

#[test]
fn resolve_music_node_playbacks_filters_inactive_track_groups() {
    let main_clip = Clip::new("main", Uuid::now_v7());
    let layer_clip = Clip::new("layer", Uuid::now_v7());
    let node_id = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut layer_track = Track::new("combat_layer", TrackRole::Layer);
    let group = TrackGroup::new("combat_layer", TrackGroupMode::Additive);
    layer_track.group = Some(group.id);

    let mut graph = MusicGraph::new("layered_music");
    graph.initial_node = Some(node_id);
    graph.groups.push(group.clone());
    graph.tracks.push(main_track.clone());
    graph.tracks.push(layer_track.clone());
    graph.nodes.push(MusicNode {
        id: node_id,
        name: "combat".into(),
        bindings: vec![
            TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: main_clip.id,
                },
            },
            TrackBinding {
                track_id: layer_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: layer_clip.id,
                },
            },
        ],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: Some(main_track.id),
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.extend([main_clip.id, layer_clip.id]);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![main_clip.clone(), layer_clip.clone()],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with layered node");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    let all_playbacks = runtime
        .resolve_music_node_playbacks(session_id, 0.0)
        .expect("node playbacks should resolve");
    assert_eq!(all_playbacks.len(), 2);

    runtime
        .set_music_track_group_active(session_id, group.id, false)
        .expect("track group should toggle");
    let filtered_playbacks = runtime
        .resolve_music_node_playbacks(session_id, 0.0)
        .expect("node playbacks should resolve");
    assert_eq!(filtered_playbacks.len(), 1);
    assert_eq!(filtered_playbacks[0].clip_id, main_clip.id);
    assert_eq!(filtered_playbacks[0].track_id, Some(main_track.id));
}

#[test]
fn activating_exclusive_group_disables_other_exclusive_groups() {
    let clip = Clip::new("main", Uuid::now_v7());
    let node_id = MusicNodeId::new();
    let mut day_track = Track::new("day_main", TrackRole::Layer);
    let mut night_track = Track::new("night_main", TrackRole::Layer);
    let day_group = TrackGroup::new("day", TrackGroupMode::Exclusive);
    let night_group = TrackGroup::new("night", TrackGroupMode::Exclusive);
    day_track.group = Some(day_group.id);
    night_track.group = Some(night_group.id);

    let mut graph = MusicGraph::new("day_night");
    graph.initial_node = Some(node_id);
    graph.groups.push(day_group.clone());
    graph.groups.push(night_group.clone());
    graph.tracks.push(day_track.clone());
    graph.tracks.push(night_track.clone());
    graph.nodes.push(MusicNode {
        id: node_id,
        name: "region".into(),
        bindings: vec![
            TrackBinding {
                track_id: day_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            },
            TrackBinding {
                track_id: night_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            },
        ],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: Some(day_track.id),
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.push(clip.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![clip],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with exclusive groups");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    runtime
        .set_music_track_group_active(session_id, night_group.id, true)
        .expect("exclusive group should toggle");

    assert_eq!(
        runtime
            .music_track_group_state(session_id, day_group.id)
            .expect("day group state should resolve"),
        TrackGroupState { active: false }
    );
    assert_eq!(
        runtime
            .music_track_group_state(session_id, night_group.id)
            .expect("night group state should resolve"),
        TrackGroupState { active: true }
    );
}

#[test]
fn resolve_music_stinger_playback_uses_active_bridge_node_stinger_track() {
    let preheat_clip = Clip::new("preheat_loop", Uuid::now_v7());
    let bridge_clip = Clip::new("boss_bridge", Uuid::now_v7());
    let boss_clip = Clip::new("boss_loop", Uuid::now_v7());
    let stinger_clip = Clip::new("boss_hit", Uuid::now_v7());
    let preheat_state = MusicNodeId::new();
    let bridge_state = MusicNodeId::new();
    let boss_state = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let bridge_track = Track::new("music_bridge", TrackRole::Bridge);
    let stinger_track = Track::new("music_stinger", TrackRole::Stinger);
    let mut graph = MusicGraph::new("boss_music");
    graph.initial_node = Some(preheat_state);
    graph.tracks.push(main_track.clone());
    graph.tracks.push(bridge_track.clone());
    graph.tracks.push(stinger_track.clone());
    graph.nodes.push(MusicNode {
        id: preheat_state,
        name: "preheat".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: preheat_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });
    graph.nodes.push(MusicNode {
        id: bridge_state,
        name: "bridge".into(),
        bindings: vec![
            TrackBinding {
                track_id: bridge_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: bridge_clip.id,
                },
            },
            TrackBinding {
                track_id: stinger_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: stinger_clip.id,
                },
            },
        ],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: false,
        completion_source: Some(bridge_track.id),
    });
    graph.nodes.push(MusicNode {
        id: boss_state,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: boss_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });
    graph.edges.push(MusicEdge {
        from: preheat_state,
        to: bridge_state,
        requested_target: Some(boss_state),
        trigger: EdgeTrigger::NextMatchingCue {
            tag: "battle_ready".into(),
        },
        destination: EntryPolicy::ClipStart,
    });
    graph.edges.push(MusicEdge {
        from: bridge_state,
        to: boss_state,
        requested_target: Some(boss_state),
        trigger: EdgeTrigger::OnComplete,
        destination: EntryPolicy::ClipStart,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.extend([
        preheat_clip.id,
        bridge_clip.id,
        boss_clip.id,
        stinger_clip.id,
    ]);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![preheat_clip, bridge_clip, boss_clip, stinger_clip.clone()],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with stinger track");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    runtime
        .request_music_node(session_id, boss_state)
        .expect("node request should succeed");
    runtime
        .complete_music_exit(session_id)
        .expect("exit cue completion should succeed");

    let stinger = runtime
        .resolve_music_stinger_playback(session_id)
        .expect("stinger playback should resolve")
        .expect("bridge node should expose stinger playback");
    assert_eq!(stinger.clip_id, stinger_clip.id);
    assert_eq!(stinger.track_id, Some(stinger_track.id));
}

#[test]
fn find_next_music_exit_cue_prefers_current_cycle_then_wraps_looping_clip() {
    let mut preheat_clip = Clip::new("preheat_loop", Uuid::now_v7());
    preheat_clip.loop_range = Some(TimeRange::new(0.0, 12.0));
    let mut cue_a = CuePoint::new("bar_1", 2.0);
    cue_a.tags.push("battle_ready".into());
    let mut cue_b = CuePoint::new("bar_2", 8.0);
    cue_b.tags.push("battle_ready".into());
    preheat_clip.cues = vec![cue_a, cue_b];

    let boss_clip = Clip::new("boss_loop", Uuid::now_v7());
    let preheat_state = MusicNodeId::new();
    let boss_state = MusicNodeId::new();
    let main_track = Track::new("music_main", TrackRole::Main);
    let mut graph = MusicGraph::new("boss_music");
    graph.initial_node = Some(preheat_state);
    graph.tracks.push(main_track.clone());
    graph.nodes.push(MusicNode {
        id: preheat_state,
        name: "preheat".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: preheat_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });
    graph.nodes.push(MusicNode {
        id: boss_state,
        name: "boss".into(),
        bindings: vec![TrackBinding {
            track_id: main_track.id,
            target: PlaybackTarget::Clip {
                clip_id: boss_clip.id,
            },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: None,
    });
    graph.edges.push(MusicEdge {
        from: preheat_state,
        to: boss_state,
        requested_target: None,
        trigger: EdgeTrigger::NextMatchingCue {
            tag: "battle_ready".into(),
        },
        destination: EntryPolicy::ClipStart,
    });

    let mut bank = Bank::new("core");
    bank.objects.clips.extend([preheat_clip.id, boss_clip.id]);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![preheat_clip, boss_clip],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load with music graph");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");
    runtime
        .request_music_node(session_id, boss_state)
        .expect("node request should succeed");

    let current_cycle = runtime
        .find_next_music_exit_cue(session_id, 3.0)
        .expect("cue lookup should succeed")
        .expect("matching cue should exist");
    assert_eq!(current_cycle.cue_position_seconds, 8.0);
    assert!(!current_cycle.requires_wrap);

    let next_cycle = runtime
        .find_next_music_exit_cue(session_id, 9.0)
        .expect("cue lookup should succeed")
        .expect("matching cue should exist");
    assert_eq!(next_cycle.cue_position_seconds, 2.0);
    assert!(next_cycle.requires_wrap);
}

#[test]
fn queued_runtime_applies_buffered_requests_against_runtime_state() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let (sampler_id, sampler) = make_sampler(asset_id);
    let event = make_event(event_id, sampler_id, vec![sampler]);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut queued = QueuedRuntime::new();
    queued
        .load_bank(bank, vec![event])
        .expect("bank should load");
    queued.queue_play(event_id);

    let results = queued
        .apply_requests()
        .expect("queued requests should apply");

    let instance_id = match results.last() {
        Some(RuntimeRequestResult::Played { instance_id }) => *instance_id,
        other => panic!("expected final played result, got {other:?}"),
    };

    assert_eq!(
        queued.active_plan(instance_id).map(|plan| &plan.asset_ids),
        Some(&vec![asset_id])
    );
}

#[test]
fn push_snapshot_creates_active_instance_and_updates_bus_volume() {
    let bus_id = BusId::new();
    let snapshot = Snapshot {
        id: SnapshotId::new(),
        name: "combat".into(),
        fade_in_seconds: 0.2,
        fade_out_seconds: 0.4,
        targets: vec![SnapshotTarget {
            bus_id,
            target_volume: 0.65,
        }],
    };
    let mut bank = Bank::new("core");
    bank.objects.buses.push(bus_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, Vec::new())
        .expect("bank should load");
    runtime.load_snapshot(snapshot.clone());

    let instance_id = runtime
        .push_snapshot(snapshot.id, Fade::seconds(snapshot.fade_in_seconds))
        .expect("snapshot should push");

    assert_eq!(runtime.bus_volume(bus_id), Some(0.65));
    let active = runtime
        .active_snapshot(instance_id)
        .expect("active snapshot should exist");
    assert_eq!(active.snapshot_id, snapshot.id);
    assert_eq!(active.overrides.get(&bus_id), Some(&0.65));
}

#[test]
fn push_snapshot_rejects_unknown_target_bus() {
    let snapshot = Snapshot {
        id: SnapshotId::new(),
        name: "combat".into(),
        fade_in_seconds: 0.2,
        fade_out_seconds: 0.4,
        targets: vec![SnapshotTarget {
            bus_id: BusId::new(),
            target_volume: 0.65,
        }],
    };

    let mut runtime = SonaraRuntime::new();
    runtime.load_snapshot(snapshot.clone());

    assert!(matches!(
        runtime.push_snapshot(snapshot.id, Fade::IMMEDIATE),
        Err(RuntimeError::SnapshotTargetBusNotFound(_))
    ));
}

#[test]
fn set_bus_gain_rejects_unknown_bus() {
    let mut runtime = SonaraRuntime::new();

    assert!(matches!(
        runtime.set_bus_gain(BusId::new(), 0.5),
        Err(RuntimeError::BusNotLoaded(_))
    ));
}

#[test]
fn set_bus_gain_updates_existing_event_bus_lookup() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let bus_id = BusId::new();
    let (sampler_id, sampler) = make_sampler(asset_id);
    let mut event = make_event(event_id, sampler_id, vec![sampler]);
    event.default_bus = Some(bus_id);

    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);
    bank.objects.buses.push(bus_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event.clone()])
        .expect("bank should load");

    let instance_id = runtime.play(event.id).expect("event should play");
    assert_eq!(runtime.active_event_bus(instance_id), Some(bus_id));
    assert_eq!(runtime.active_bus_gain(instance_id), Some(1.0));

    runtime
        .set_bus_gain(bus_id, 0.35)
        .expect("bus gain should update");

    assert_eq!(runtime.bus_gain(bus_id), Some(0.35));
    assert_eq!(runtime.active_bus_gain(instance_id), Some(0.35));
}

#[test]
fn set_bus_effect_slot_updates_loaded_live_slot_state() {
    let mut bus = Bus::new("sfx");
    let mut original_slot = BusEffectSlot::low_pass(1_200.0);
    original_slot
        .low_pass_effect_mut()
        .expect("slot should be low-pass")
        .enabled = false;
    let slot_id = original_slot.id;
    bus.effect_slots.push(original_slot.clone());

    let mut bank = Bank::new("core");
    bank.objects.buses.push(bus.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            vec![bus.clone()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("bank should load");

    let mut updated_slot = original_slot.clone();
    let low_pass = updated_slot
        .low_pass_effect_mut()
        .expect("slot should be low-pass");
    low_pass.enabled = true;
    low_pass.set_cutoff_hz(480.0);

    runtime
        .set_bus_effect_slot(bus.id, updated_slot.clone())
        .expect("bus effect slot should update");

    let live_slots = runtime
        .bus_effect_slots(bus.id)
        .expect("live effect slots should exist");
    let live_slot = live_slots
        .iter()
        .find(|slot| slot.id == slot_id)
        .expect("updated slot should be present");

    assert_eq!(live_slot, &updated_slot);
}

#[test]
fn active_bus_volume_follows_event_default_bus_override() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let bus_id = BusId::new();
    let (sampler_id, sampler) = make_sampler(asset_id);
    let mut event = make_event(event_id, sampler_id, vec![sampler]);
    event.default_bus = Some(bus_id);

    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);
    bank.objects.buses.push(bus_id);

    let snapshot = Snapshot {
        id: SnapshotId::new(),
        name: "combat".into(),
        fade_in_seconds: 0.2,
        fade_out_seconds: 0.4,
        targets: vec![SnapshotTarget {
            bus_id,
            target_volume: 0.4,
        }],
    };

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event.clone()])
        .expect("bank should load");
    runtime.load_snapshot(snapshot.clone());
    runtime
        .push_snapshot(snapshot.id, Fade::IMMEDIATE)
        .expect("snapshot should push");

    let instance_id = runtime.play(event.id).expect("event should play");

    assert_eq!(runtime.active_bus_volume(instance_id), Some(0.4));
}

#[test]
fn music_track_output_bus_resolves_from_active_session() {
    let asset_id = Uuid::now_v7();
    let bus = Bus::new("music");
    let clip = Clip::new("explore", asset_id);
    let mut track = Track::new("music_main", TrackRole::Main);
    track.output_bus = Some(bus.id);
    let node_id = MusicNodeId::new();
    let mut graph = MusicGraph::new("music");
    graph.initial_node = Some(node_id);
    graph.tracks.push(track.clone());
    graph.nodes.push(MusicNode {
        id: node_id,
        name: "explore".into(),
        bindings: vec![TrackBinding {
            track_id: track.id,
            target: PlaybackTarget::Clip { clip_id: clip.id },
        }],
        memory_slot: None,
        memory_policy: MemoryPolicy::default(),
        default_entry: EntryPolicy::ClipStart,
        externally_targetable: true,
        completion_source: Some(track.id),
    });

    let mut bank = Bank::new("core");
    bank.objects.buses.push(bus.id);
    bank.objects.clips.push(clip.id);
    bank.objects.music_graphs.push(graph.id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            vec![bus.clone()],
            Vec::new(),
            vec![clip],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("bank should load");

    let session_id = runtime
        .play_music_graph(graph.id)
        .expect("music graph should start");

    assert_eq!(
        runtime
            .music_track_output_bus(session_id, track.id)
            .expect("track output bus should resolve"),
        Some(bus.id)
    );
}
