use std::{thread, time::Duration};

use sonara_build::compile_bank_definition;
use sonara_firewheel::{FirewheelBackend, FirewheelRequestResult};
use sonara_model::{AuthoringProject, ParameterValue};
use sonara_runtime::Fade;

fn main() {
    let project = AuthoringProject::read_json_file("sonara-app/assets/demo/project.json")
        .expect("demo project should load from JSON");
    let bank_definition = project
        .bank_named("core")
        .expect("demo project should contain core bank");
    let package =
        compile_bank_definition(bank_definition, &project).expect("demo project should compile");
    let event_id = package.events[0].id;
    let surface_id = project.parameters[0].id();
    let wood_asset = package.bank.manifest.assets[0].id;
    let stone_asset = package.bank.manifest.assets[1].id;
    let wood_path = package.bank.manifest.assets[0].source_path.clone();
    let stone_path = package.bank.manifest.assets[1].source_path.clone();

    let mut backend =
        FirewheelBackend::new(Default::default()).expect("Firewheel backend should start");
    backend
        .load_compiled_bank(package)
        .expect("compiled demo package should decode and load");

    let emitter_id = backend.runtime_mut().create_emitter();
    backend.queue_set_emitter_param(emitter_id, surface_id, ParameterValue::Enum("stone".into()));
    backend.queue_play_on(emitter_id, event_id);

    let request_results = backend
        .apply_requests()
        .expect("queued requests should apply");
    let instance_id = match request_results.last() {
        Some(FirewheelRequestResult::Played { instance_id }) => *instance_id,
        other => panic!("expected final played result, got {other:?}"),
    };
    let plan = backend
        .runtime()
        .active_plan(instance_id)
        .expect("active plan should exist");
    let resolved_label = match plan.asset_ids.as_slice() {
        [asset_id] if *asset_id == wood_asset => "wood",
        [asset_id] if *asset_id == stone_asset => "stone",
        _ => "unknown",
    };

    println!("Sonara demo");
    println!("event: player.footstep");
    println!("emitter: {:?}", plan.emitter_id);
    println!("project file: sonara-app/assets/demo/project.json");
    println!("surface param: stone");
    println!("wood file: {}", wood_path);
    println!("stone file: {}", stone_path);
    println!("wood asset: {:?}", wood_asset);
    println!("stone asset: {:?}", stone_asset);
    println!("resolved branch: {resolved_label}");
    println!("resolved assets: {:?}", plan.asset_ids);
    println!("request results: {:?}", request_results);
    println!("playing for 100ms before stop...");

    for _ in 0..1 {
        backend.update().expect("backend update should succeed");
        thread::sleep(Duration::from_millis(100));
    }

    backend.queue_stop(instance_id, Fade::IMMEDIATE);
    let stop_results = backend.apply_requests().expect("queued stop should apply");
    let plan_after_stop = backend.runtime().active_plan(instance_id);

    println!("stop request results: {:?}", stop_results);
    println!("active plan after stop: {:?}", plan_after_stop);
    println!("draining backend for 400ms after stop...");

    for _ in 0..4 {
        backend.update().expect("backend update should succeed");
        thread::sleep(Duration::from_millis(100));
    }
}
