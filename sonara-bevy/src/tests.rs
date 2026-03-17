// SPDX-License-Identifier: MPL-2.0

use bevy_app::{App, Update};
use bevy_ecs::{
    prelude::{Entity, NonSendMut},
    system::Single,
};
use sonara_model::{
    Bank, Clip, EdgeTrigger, EntryPolicy, Event, EventContentRoot, EventId, EventKind,
    MemoryPolicy, MusicEdge, MusicGraph, MusicNode, MusicNodeId, NodeId, NodeRef, ParameterId,
    ParameterValue, PlaybackTarget, SamplerNode, SpatialMode, SwitchCase, SwitchNode, Track,
    TrackBinding, TrackRole,
};
use sonara_runtime::{EventInstanceId, Fade, RuntimeError};
use uuid::Uuid;

use super::*;

fn make_switch_event(event_id: EventId, parameter_id: ParameterId, asset_id: Uuid) -> Event {
    let switch_id = NodeId::new();
    let sampler_id = NodeId::new();

    Event {
        id: event_id,
        name: "player.footstep".into(),
        kind: EventKind::OneShot,
        root: EventContentRoot {
            root: NodeRef { id: switch_id },
            nodes: vec![
                sonara_model::EventContentNode::Switch(SwitchNode {
                    id: switch_id,
                    parameter_id,
                    cases: vec![SwitchCase {
                        variant: "stone".into(),
                        child: NodeRef { id: sampler_id },
                    }],
                    default_case: Some(NodeRef { id: sampler_id }),
                }),
                sonara_model::EventContentNode::Sampler(SamplerNode {
                    id: sampler_id,
                    asset_id,
                }),
            ],
        },
        default_bus: None,
        spatial: SpatialMode::ThreeD,
        default_parameters: Vec::new(),
        voice_limit: None,
        steal_policy: None,
    }
}

fn make_music_graph() -> (Clip, Clip, Clip, MusicGraph, MusicNodeId, MusicNodeId) {
    let preheat_clip = Clip::new("preheat_loop", Uuid::now_v7());
    let bridge_clip = Clip::new("preheat_to_boss", Uuid::now_v7());
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

    (
        preheat_clip,
        bridge_clip,
        boss_clip,
        graph,
        preheat_state,
        boss_state,
    )
}

#[test]
fn ensure_emitter_reuses_existing_id() {
    let mut audio = SonaraAudio::new();
    let mut emitter = AudioEmitter::default();

    let first = audio.ensure_emitter(&mut emitter);
    let second = audio.ensure_emitter(&mut emitter);

    assert_eq!(Some(first), emitter.id);
    assert_eq!(first, second);
}

#[test]
fn detach_emitter_clears_bound_id() {
    let mut audio = SonaraAudio::new();
    let mut emitter = AudioEmitter::default();

    let _ = audio.ensure_emitter(&mut emitter);
    audio
        .detach_emitter(&mut emitter)
        .expect("detach should succeed");

    assert_eq!(None, emitter.id);
}

#[test]
fn play_from_emitter_uses_component_bound_emitter() {
    let surface_id = ParameterId::new();
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let event = make_switch_event(event_id, surface_id, asset_id);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut audio = SonaraAudio::new();
    audio
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let mut emitter = AudioEmitter::default();
    audio
        .set_emitter_param_on(
            &mut emitter,
            surface_id,
            ParameterValue::Enum("stone".into()),
        )
        .expect("emitter param should set");

    let instance_id = audio
        .play_from_emitter(&mut emitter, event_id)
        .expect("play should succeed");
    let plan = audio.active_plan(instance_id).expect("plan should exist");

    assert_eq!(plan.emitter_id, emitter.id);
    assert_eq!(plan.asset_ids, vec![asset_id]);
}

#[test]
fn queued_requests_are_applied_in_order() {
    let surface_id = ParameterId::new();
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let event = make_switch_event(event_id, surface_id, asset_id);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut audio = SonaraAudio::new();
    audio
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let mut emitter = AudioEmitter::default();
    audio.queue_set_emitter_param_on(
        &mut emitter,
        surface_id,
        ParameterValue::Enum("stone".into()),
    );
    audio.queue_play_from_emitter(&mut emitter, event_id);

    let results = audio.apply_requests().expect("requests should apply");
    let instance_id = match results.last() {
        Some(AudioRequestResult::Played { instance_id }) => *instance_id,
        other => panic!("expected final played result, got {other:?}"),
    };
    let plan = audio.active_plan(instance_id).expect("plan should exist");

    assert_eq!(results.len(), 2);
    assert_eq!(plan.emitter_id, emitter.id);
    assert_eq!(plan.asset_ids, vec![asset_id]);
}

#[test]
fn isolated_request_application_keeps_processing_after_error() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let surface_id = ParameterId::new();
    let event = make_switch_event(event_id, surface_id, asset_id);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut audio = SonaraAudio::new();
    audio
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let missing_emitter = audio.create_emitter();
    let mut detached = AudioEmitter {
        enabled: true,
        id: Some(missing_emitter),
    };
    audio
        .detach_emitter(&mut detached)
        .expect("detach should succeed");

    audio.queue_play_on(missing_emitter, event_id);
    audio.queue_play(event_id);

    let outcomes = audio.apply_requests_isolated();

    assert_eq!(outcomes.len(), 2);
    assert!(matches!(
        outcomes[0].result,
        Err(AudioBackendError::Runtime(RuntimeError::EmitterNotFound(_)))
    ));
    assert!(matches!(
        outcomes[1].result,
        Ok(AudioRequestResult::Played { .. })
    ));
}

#[test]
fn queued_stop_request_removes_active_instance() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let surface_id = ParameterId::new();
    let event = make_switch_event(event_id, surface_id, asset_id);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut audio = SonaraAudio::new();
    audio
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let instance_id = audio.play(event_id).expect("play should succeed");
    audio.queue_stop(instance_id, Fade::IMMEDIATE);

    let results = audio.apply_requests().expect("requests should apply");

    assert_eq!(results, vec![AudioRequestResult::Stopped { instance_id }]);
    assert_eq!(audio.active_plan(instance_id), None);
}

#[test]
fn instance_state_reports_runtime_playback_lifecycle() {
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let surface_id = ParameterId::new();
    let event = make_switch_event(event_id, surface_id, asset_id);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut audio = SonaraAudio::new();
    audio
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let instance_id = audio.play(event_id).expect("play should succeed");
    assert_eq!(
        audio.instance_state(instance_id),
        EventInstanceState::Playing
    );

    audio
        .stop(instance_id, Fade::IMMEDIATE)
        .expect("stop should succeed");
    assert_eq!(
        audio.instance_state(instance_id),
        EventInstanceState::Stopped
    );
}

#[test]
fn update_context_batches_emitter_commands_and_applies_them() {
    let surface_id = ParameterId::new();
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let event = make_switch_event(event_id, surface_id, asset_id);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut audio = SonaraAudio::new();
    audio
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let mut emitter = AudioEmitter::default();
    let results = {
        let mut update = audio.begin_update();
        update.set_emitter_param_on(
            &mut emitter,
            surface_id,
            ParameterValue::Enum("stone".into()),
        );
        update.play_from_emitter(&mut emitter, event_id);
        update.apply().expect("update should apply")
    };

    let instance_id = match results.last() {
        Some(AudioRequestResult::Played { instance_id }) => *instance_id,
        other => panic!("expected final played result, got {other:?}"),
    };
    let plan = audio.active_plan(instance_id).expect("plan should exist");

    assert_eq!(results.len(), 2);
    assert_eq!(plan.emitter_id, emitter.id);
    assert_eq!(plan.asset_ids, vec![asset_id]);
}

#[test]
fn plugin_exposes_audio_resource_to_real_bevy_update_system() {
    fn bevy_audio_system(
        mut audio: NonSendMut<SonaraAudio>,
        mut emitter: Single<&mut AudioEmitter>,
        event_id: NonSendMut<TestEventId>,
        surface_id: NonSendMut<TestSurfaceId>,
        mut played: NonSendMut<PlayedInstance>,
    ) {
        let mut update = audio.begin_update();
        update.set_emitter_param_on(
            &mut emitter,
            surface_id.0,
            ParameterValue::Enum("stone".into()),
        );
        update.play_from_emitter(&mut emitter, event_id.0);
        let results = update.apply().expect("update should apply");

        *played = PlayedInstance(match results.last() {
            Some(AudioRequestResult::Played { instance_id }) => Some(*instance_id),
            other => panic!("expected final played result, got {other:?}"),
        });
    }

    struct TestEventId(EventId);

    struct TestSurfaceId(ParameterId);

    #[derive(Default)]
    struct PlayedInstance(Option<EventInstanceId>);

    let surface_id = ParameterId::new();
    let event_id = EventId::new();
    let asset_id = Uuid::now_v7();
    let event = make_switch_event(event_id, surface_id, asset_id);
    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);

    let mut app = App::new();
    app.add_plugins(SonaraPlugin);
    app.insert_non_send_resource(TestEventId(event_id));
    app.insert_non_send_resource(TestSurfaceId(surface_id));
    app.insert_non_send_resource(PlayedInstance::default());
    app.world_mut().spawn(AudioEmitter::default());
    app.world_mut()
        .non_send_resource_mut::<SonaraAudio>()
        .load_bank(bank, vec![event])
        .expect("bank should load");
    app.add_systems(Update, bevy_audio_system);

    app.update();

    let played = app.world().non_send_resource::<PlayedInstance>().0;
    let instance_id = played.expect("system should have played an instance");
    let plan = app
        .world()
        .non_send_resource::<SonaraAudio>()
        .active_plan(instance_id)
        .expect("plan should exist");
    let plan_emitter_id = plan.emitter_id;
    let plan_asset_ids = plan.asset_ids.clone();
    let emitter_id = {
        let mut query = app.world_mut().query::<(Entity, &AudioEmitter)>();
        query
            .single(app.world())
            .expect("there should be one emitter entity")
            .1
            .id
    };

    assert_eq!(emitter_id, plan_emitter_id);
    assert_eq!(plan_asset_ids, vec![asset_id]);
}

#[test]
fn music_graph_api_tracks_transition_lifecycle_in_runtime_mode() {
    let (preheat_clip, bridge_clip, boss_clip, graph, preheat_state, boss_state) =
        make_music_graph();
    let mut bank = Bank::new("music");
    bank.objects
        .clips
        .extend([preheat_clip.id, bridge_clip.id, boss_clip.id]);
    bank.objects.music_graphs.push(graph.id);

    let mut audio = SonaraAudio::new();
    audio
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![preheat_clip, bridge_clip, boss_clip],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("music bank should load");

    let session_id = audio
        .play_music_graph(graph.id)
        .expect("music graph should start");
    let status = audio
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(status.active_node, preheat_state);
    assert_eq!(status.phase, MusicPhase::Stable);

    audio
        .request_music_node(session_id, boss_state)
        .expect("music node request should succeed");
    let status = audio
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(status.desired_target_node, boss_state);
    assert_eq!(status.phase, MusicPhase::WaitingExitCue);

    audio
        .complete_music_exit(session_id)
        .expect("exit cue completion should succeed");
    let status = audio
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(status.phase, MusicPhase::WaitingNodeCompletion);

    audio
        .complete_music_node_completion(session_id)
        .expect("bridge completion should succeed");
    let status = audio
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(status.active_node, boss_state);
    assert_eq!(status.phase, MusicPhase::Stable);
}

#[test]
fn stop_music_session_marks_session_stopped() {
    let (preheat_clip, bridge_clip, boss_clip, graph, _, _) = make_music_graph();
    let mut bank = Bank::new("music");
    bank.objects
        .clips
        .extend([preheat_clip.id, bridge_clip.id, boss_clip.id]);
    bank.objects.music_graphs.push(graph.id);

    let mut audio = SonaraAudio::new();
    audio
        .load_bank_with_definitions(
            bank,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![preheat_clip, bridge_clip, boss_clip],
            Vec::new(),
            Vec::new(),
            vec![graph.clone()],
        )
        .expect("music bank should load");

    let session_id = audio
        .play_music_graph(graph.id)
        .expect("music graph should start");
    audio
        .stop_music_session(session_id, Fade::IMMEDIATE)
        .expect("stopping music session should succeed");

    let status = audio
        .music_status(session_id)
        .expect("music status should resolve");
    assert_eq!(status.phase, MusicPhase::Stopped);
}
