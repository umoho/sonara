use bevy_app::{App, Startup, Update};
use bevy_ecs::prelude::{Commands, Query, Res, ResMut, Resource};
use sonara_bevy::prelude::{AudioEmitter, SonaraAudio, SonaraPlugin};
use sonara_model::{
    Bank, Event, EventContentNode, EventContentRoot, EventId, EventKind, NodeId, NodeRef,
    ParameterId, ParameterValue, SamplerNode, SpatialMode, SwitchCase, SwitchNode,
};
use sonara_runtime::{Fade, PlaybackPlan};
use uuid::Uuid;

#[derive(Resource)]
struct DemoIds {
    event_id: EventId,
    surface_id: ParameterId,
    wood_asset: Uuid,
    stone_asset: Uuid,
}

#[derive(Resource, Default)]
struct DemoState {
    frame: u32,
    instance_id: Option<sonara_runtime::EventInstanceId>,
}

fn main() {
    let event_id = EventId::new();
    let surface_id = ParameterId::new();
    let wood_asset = Uuid::now_v7();
    let stone_asset = Uuid::now_v7();

    let mut app = App::new();
    app.add_plugins(SonaraPlugin);
    app.insert_resource(DemoIds {
        event_id,
        surface_id,
        wood_asset,
        stone_asset,
    });
    app.insert_resource(DemoState::default());
    app.add_systems(Startup, setup_audio_demo);
    app.add_systems(Update, run_audio_demo);

    println!("Sonara Bevy example");
    println!("this example runs real bevy_app/bevy_ecs integration");
    println!("it resolves playback plans but does not start Firewheel audio output");

    for _ in 0..3 {
        app.update();
    }
}

fn setup_audio_demo(
    mut commands: Commands,
    mut audio: ResMut<SonaraAudio>,
    demo_ids: Res<DemoIds>,
) {
    let switch_id = NodeId::new();
    let wood_node_id = NodeId::new();
    let stone_node_id = NodeId::new();
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

    let mut bank = Bank::new("core");
    bank.events.push(demo_ids.event_id);
    audio
        .load_bank(bank, vec![event])
        .expect("bank should load in startup system");

    commands.spawn(AudioEmitter::default());
}

fn run_audio_demo(
    mut audio: ResMut<SonaraAudio>,
    mut demo_state: ResMut<DemoState>,
    demo_ids: Res<DemoIds>,
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
                Some(sonara_bevy::AudioRequestResult::Played { instance_id }) => *instance_id,
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
            let instance_id = demo_state.instance_id.expect("instance should exist");
            let results = {
                let mut update = audio.begin_update();
                update.stop(instance_id, Fade::IMMEDIATE);
                update.apply().expect("stop update should apply")
            };

            println!("frame 1 results: {:?}", results);
            println!(
                "frame 1 active plan after stop: {:?}",
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
