use std::{thread, time::Duration};

use bevy_app::{App, Startup, Update};
use bevy_ecs::prelude::{Commands, NonSend, NonSendMut, Query};
use sonara_bevy::{
    AudioRequestResult,
    prelude::{AudioEmitter, SonaraAudio, SonaraFirewheelPlugin},
};
use sonara_build::CompiledBankPackage;
use sonara_model::{EventId, ParameterId, ParameterValue};
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
    let demo_ids = read_demo_ids();

    let mut app = App::new();
    app.add_plugins(SonaraFirewheelPlugin);
    app.insert_non_send_resource(demo_ids);
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

fn read_demo_ids() -> DemoIds {
    let package = CompiledBankPackage::read_json_file("sonara-app/assets/demo/core.bank.json")
        .expect("compiled demo bank should load from JSON");
    let event = package
        .events()
        .iter()
        .find(|event| event.name == "player.footstep")
        .expect("compiled bank should contain player.footstep");
    let wood_asset = package
        .bank()
        .manifest
        .assets
        .iter()
        .find(|asset| asset.name == "footstep_wood")
        .expect("compiled bank should contain wood asset");
    let stone_asset = package
        .bank()
        .manifest
        .assets
        .iter()
        .find(|asset| asset.name == "footstep_stone")
        .expect("compiled bank should contain stone asset");

    DemoIds {
        event_id: event.id,
        surface_id: *event
            .default_parameters
            .first()
            .expect("compiled footstep event should reference a surface parameter"),
        wood_asset: wood_asset.id,
        stone_asset: stone_asset.id,
    }
}

fn setup_audio_demo(
    mut commands: Commands,
    mut audio: NonSendMut<SonaraAudio>,
    demo_ids: NonSend<DemoIds>,
) {
    let package = CompiledBankPackage::read_json_file("sonara-app/assets/demo/core.bank.json")
        .expect("compiled demo bank should load from JSON");
    let wood_asset = package
        .bank()
        .manifest
        .assets
        .iter()
        .find(|asset| asset.id == demo_ids.wood_asset)
        .expect("compiled bank should contain wood asset");
    let stone_asset = package
        .bank()
        .manifest
        .assets
        .iter()
        .find(|asset| asset.id == demo_ids.stone_asset)
        .expect("compiled bank should contain stone asset");
    let wood_path = wood_asset.source_path.clone();
    let stone_path = stone_asset.source_path.clone();
    audio
        .load_compiled_bank(package)
        .expect("compiled bank should load in startup system");

    println!("compiled bank file: sonara-app/assets/demo/core.bank.json");
    println!("wood file: {}", wood_path);
    println!("stone file: {}", stone_path);
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
