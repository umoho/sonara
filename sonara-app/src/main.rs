use std::{f32::consts::PI, thread, time::Duration};

use sonara_firewheel::FirewheelBackend;
use sonara_model::{
    Bank, Event, EventContentNode, EventContentRoot, EventId, EventKind, NodeId, NodeRef,
    ParameterId, ParameterValue, SamplerNode, SpatialMode, SwitchCase, SwitchNode,
};
use sonara_runtime::SonaraRuntime;
use uuid::Uuid;

fn main() {
    let surface_id = ParameterId::new();
    let event_id = EventId::new();
    let switch_id = NodeId::new();
    let wood_asset = Uuid::now_v7();
    let stone_asset = Uuid::now_v7();
    let wood_node_id = NodeId::new();
    let stone_node_id = NodeId::new();

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

    let mut bank = Bank::new("core");
    bank.events.push(event_id);

    let mut runtime = SonaraRuntime::new();
    runtime
        .load_bank(bank, vec![event])
        .expect("bank should load");

    let mut backend = FirewheelBackend::new(runtime).expect("Firewheel backend should start");
    backend
        .register_interleaved_f32_asset(wood_asset, 2, 48_000, generate_tone(320.0, 0.18, 0.18))
        .expect("wood asset should register");
    backend
        .register_interleaved_f32_asset(stone_asset, 2, 48_000, generate_tone(880.0, 0.09, 0.12))
        .expect("stone asset should register");

    let emitter_id = backend.runtime_mut().create_emitter();
    backend
        .runtime_mut()
        .set_emitter_param(emitter_id, surface_id, ParameterValue::Enum("stone".into()))
        .expect("emitter param should set");

    let instance_id = backend
        .play_on(emitter_id, event_id)
        .expect("event should play on emitter");
    let plan = backend
        .runtime()
        .active_plan(instance_id)
        .expect("active plan should exist");

    println!("Sonara demo");
    println!("event: player.footstep");
    println!("emitter: {:?}", plan.emitter_id);
    println!("resolved assets: {:?}", plan.asset_ids);
    println!("playing for 2 seconds...");

    for _ in 0..20 {
        backend.update().expect("backend update should succeed");
        thread::sleep(Duration::from_millis(100));
    }
}

/// 生成一个带指数衰减包络的最小测试音色
fn generate_tone(frequency_hz: f32, duration_seconds: f32, amplitude: f32) -> Vec<f32> {
    let sample_rate = 48_000.0;
    let frames = (duration_seconds * sample_rate) as usize;
    let mut data = Vec::with_capacity(frames * 2);

    for frame in 0..frames {
        let t = frame as f32 / sample_rate;
        let envelope = (1.0 - t / duration_seconds).max(0.0).powf(2.5);
        let sample = (2.0 * PI * frequency_hz * t).sin() * amplitude * envelope;
        data.push(sample);
        data.push(sample);
    }

    data
}
