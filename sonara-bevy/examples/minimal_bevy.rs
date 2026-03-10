use std::{thread, time::Duration};

use bevy_app::{App, Startup, Update};
use bevy_ecs::prelude::{Commands, NonSend, NonSendMut, Query};
use camino::Utf8PathBuf;
use sonara_bevy::{
    AudioRequestResult,
    prelude::{AudioEmitter, SonaraAudio, SonaraFirewheelPlugin},
};
use sonara_build::build_bank;
use sonara_model::{
    AudioAsset, Event, EventContentNode, EventContentRoot, EventId, EventKind, NodeId, NodeRef,
    ParameterId, ParameterValue, SamplerNode, SpatialMode, SwitchCase, SwitchNode,
};
use sonara_runtime::{EventInstanceId, Fade, PlaybackPlan};
use uuid::Uuid;

struct DemoIds {
    event_id: EventId,
    surface_id: ParameterId,
    wood_asset: Uuid,
    stone_asset: Uuid,
}

#[derive(Default)]
struct DemoState {
    frame: u32,
    instance_id: Option<EventInstanceId>,
}

fn main() {
    let event_id = EventId::new();
    let surface_id = ParameterId::new();
    let wood_asset = Uuid::now_v7();
    let stone_asset = Uuid::now_v7();

    let mut app = App::new();
    app.add_plugins(SonaraFirewheelPlugin);
    app.insert_non_send_resource(DemoIds {
        event_id,
        surface_id,
        wood_asset,
        stone_asset,
    });
    app.insert_non_send_resource(DemoState::default());
    app.add_systems(Startup, setup_audio_demo);
    app.add_systems(Update, run_audio_demo);

    println!("Sonara Bevy example");
    println!("this example runs real bevy_app/bevy_ecs + firewheel audio output");

    for _ in 0..6 {
        app.update();
        thread::sleep(Duration::from_millis(100));
    }
}

fn setup_audio_demo(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
) {
    let switch_id = NodeId::new();
    let wood_node_id = NodeId::new();
    let stone_node_id = NodeId::new();
    let wood_path = Utf8PathBuf::from("sonara-app/assets/demo/footstep_wood.wav");
    let stone_path = Utf8PathBuf::from("sonara-app/assets/demo/footstep_stone.wav");

    let mut wood_audio_asset = AudioAsset::new("footstep_wood", wood_path);
    wood_audio_asset.id = demo_ids.wood_asset;
    let mut stone_audio_asset = AudioAsset::new("footstep_stone", stone_path);
    stone_audio_asset.id = demo_ids.stone_asset;

    let event = Event {
        id: demo_ids.event_id,
        name: "player.footstep".into(),
        kind: EventKind::OneShot,
        root: EventContentRoot {
            root: NodeRef { id: switch_id },
            nodes: vec![
                EventContentNode::Switch(SwitchNode {
                    id: switch_id,
                    parameter_id: demo_ids.surface_id,
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
                EventContentNode::Sampler(SamplerNode {
                    id: wood_node_id,
                    asset_id: demo_ids.wood_asset,
                }),
                EventContentNode::Sampler(SamplerNode {
                    id: stone_node_id,
                    asset_id: demo_ids.stone_asset,
                }),
            ],
        },
        default_bus: None,
        spatial: SpatialMode::ThreeD,
        default_parameters: Vec::new(),
        voice_limit: None,
        steal_policy: None,
    };

    let bank = build_bank(
        "core",
        &[event.clone()],
        &[wood_audio_asset.clone(), stone_audio_asset.clone()],
    )
    .expect("bank should build");

    audio
        .load_bank(bank, vec![event])
        .expect("bank should load in startup system");

    println!("wood file: {}", wood_audio_asset.source_path);
    println!("stone file: {}", stone_audio_asset.source_path);
    println!("wood asset: {:?}", demo_ids.wood_asset);
    println!("stone asset: {:?}", demo_ids.stone_asset);

    commands.spawn(AudioEmitter::default());
}

fn run_audio_demo(
    mut audio: NonSendMut<SonaraAudio>,
    mut demo_state: NonSendMut<DemoState>,
    demo_ids: NonSend<DemoIds>,
    mut emitters: Query<&mut AudioEmitter>,
) {
    let mut emitter = emitters.single_mut().expect("there should be one emitter");

    match demo_state.frame {
        0 => {
            let results = {
                let mut update = audio.begin_update();
                update.set_emitter_param_on(
                    &mut emitter,
                    demo_ids.surface_id,
                    ParameterValue::Enum("stone".into()),
                );
                update.play_from_emitter(&mut emitter, demo_ids.event_id);
                update.apply().expect("play update should apply")
            };

            let instance_id = match results.last() {
                Some(AudioRequestResult::Played { instance_id }) => *instance_id,
                other => panic!("expected played result, got {other:?}"),
            };
            demo_state.instance_id = Some(instance_id);

            let plan = audio
                .active_plan(instance_id)
                .expect("plan should exist after play");
            print_plan("frame 0 play", plan, demo_ids.stone_asset);
            println!("frame 0 results: {:?}", results);
        }
        1 => {
            println!("frame 1 letting firewheel play for another 100ms");
        }
        2 => {
            let instance_id = demo_state.instance_id.expect("instance should exist");
            let results = {
                let mut update = audio.begin_update();
                update.stop(instance_id, Fade::IMMEDIATE);
                update.apply().expect("stop update should apply")
            };

            println!("frame 2 results: {:?}", results);
            println!(
                "frame 2 active plan after stop: {:?}",
                audio.active_plan(instance_id)
            );
        }
        _ => {
            println!("frame {} idle", demo_state.frame);
        }
    }

    demo_state.frame += 1;
}

fn print_plan(label: &str, plan: &PlaybackPlan, stone_asset: Uuid) {
    let resolved_branch = match plan.asset_ids.as_slice() {
        [asset_id] if *asset_id == stone_asset => "stone",
        [_] => "wood",
        _ => "unknown",
    };

    println!("{label}: emitter={:?}", plan.emitter_id);
    println!("{label}: resolved branch={resolved_branch}");
    println!("{label}: resolved assets={:?}", plan.asset_ids);
}
