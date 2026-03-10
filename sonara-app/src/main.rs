use std::{thread, time::Duration};

use camino::Utf8PathBuf;
use sonara_build::build_bank;
use sonara_firewheel::{FirewheelBackend, FirewheelRequestResult};
use sonara_model::{
    AudioAsset, Event, EventContentNode, EventContentRoot, EventId, EventKind, NodeId, NodeRef,
    ParameterId, ParameterValue, SamplerNode, SpatialMode, SwitchCase, SwitchNode,
};
use uuid::Uuid;

fn main() {
    let surface_id = ParameterId::new();
    let event_id = EventId::new();
    let switch_id = NodeId::new();
    let wood_asset = Uuid::now_v7();
    let stone_asset = Uuid::now_v7();
    let wood_node_id = NodeId::new();
    let stone_node_id = NodeId::new();
    let wood_path = Utf8PathBuf::from("sonara-app/assets/demo/footstep_wood.wav");
    let stone_path = Utf8PathBuf::from("sonara-app/assets/demo/footstep_stone.wav");

    let mut wood_audio_asset = AudioAsset::new("footstep_wood", wood_path);
    wood_audio_asset.id = wood_asset;
    let mut stone_audio_asset = AudioAsset::new("footstep_stone", stone_path);
    stone_audio_asset.id = stone_asset;

    let event = Event {
        id: event_id,
        name: "player.footstep".into(),
        kind: EventKind::OneShot,
        root: EventContentRoot {
            root: NodeRef { id: switch_id },
            nodes: vec![
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
                EventContentNode::Sampler(SamplerNode {
                    id: wood_node_id,
                    asset_id: wood_asset,
                }),
                EventContentNode::Sampler(SamplerNode {
                    id: stone_node_id,
                    asset_id: stone_asset,
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

    let mut backend =
        FirewheelBackend::new(Default::default()).expect("Firewheel backend should start");
    backend
        .load_bank(bank, vec![event])
        .expect("bank assets should decode and load");

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
    println!("surface param: stone");
    println!("wood file: {}", wood_audio_asset.source_path);
    println!("stone file: {}", stone_audio_asset.source_path);
    println!("wood asset: {:?}", wood_asset);
    println!("stone asset: {:?}", stone_asset);
    println!("resolved branch: {resolved_label}");
    println!("resolved assets: {:?}", plan.asset_ids);
    println!("request results: {:?}", request_results);
    println!("playing for 2 seconds...");

    for _ in 0..20 {
        backend.update().expect("backend update should succeed");
        thread::sleep(Duration::from_millis(100));
    }
}
