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

    let emitter_id = runtime.create_emitter();
    runtime
        .set_emitter_param(emitter_id, surface_id, ParameterValue::Enum("stone".into()))
        .expect("emitter param should set");

    let instance_id = runtime
        .play_on(emitter_id, event_id)
        .expect("event should play on emitter");
    let plan = runtime
        .active_plan(instance_id)
        .expect("active plan should exist");

    println!("Sonara demo");
    println!("event: player.footstep");
    println!("emitter: {:?}", plan.emitter_id);
    println!("resolved assets: {:?}", plan.asset_ids);
}
