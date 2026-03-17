use sonara_model::{
    Bank, Bus, BusEffectSlot, Event, EventContentNode, EventContentRoot, EventId, EventKind,
    NodeId, NodeRef, SamplerNode, SpatialMode,
};
use uuid::Uuid;

use crate::FirewheelBackend;

fn make_sampler(asset_id: Uuid) -> (NodeId, EventContentNode) {
    let id = NodeId::new();
    (id, EventContentNode::Sampler(SamplerNode { id, asset_id }))
}

fn make_event(event_id: EventId, root: NodeId, nodes: Vec<EventContentNode>, bus: Bus) -> Event {
    Event {
        id: event_id,
        name: "test.footstep".into(),
        kind: EventKind::OneShot,
        root: EventContentRoot {
            root: NodeRef { id: root },
            nodes,
        },
        default_bus: Some(bus.id),
        spatial: SpatialMode::TwoD,
        default_parameters: Vec::new(),
        voice_limit: None,
        steal_policy: None,
    }
}

#[test]
fn live_bus_low_pass_updates_existing_worker() {
    let mut backend = FirewheelBackend::new(Default::default())
        .expect("firewheel backend should start for local regression test");

    let asset_id = Uuid::now_v7();
    backend
        .register_interleaved_f32_asset(asset_id, 1, 48_000, vec![0.0; 48_000])
        .expect("synthetic sample should register");

    let mut bus = Bus::new("sfx_underwater");
    let mut low_pass_slot = BusEffectSlot::low_pass(450.0);
    low_pass_slot
        .low_pass_effect_mut()
        .expect("slot should be low-pass")
        .enabled = false;
    bus.effect_slots.push(low_pass_slot.clone());

    let event_id = EventId::new();
    let (sampler_id, sampler) = make_sampler(asset_id);
    let event = make_event(event_id, sampler_id, vec![sampler], bus.clone());

    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);
    bank.objects.buses.push(bus.id);

    backend
        .load_bank_with_definitions(
            bank,
            vec![event],
            vec![bus.clone()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("bank should load");

    let instance_id = backend.play(event_id).expect("event should play");
    let worker_id = *backend
        .instance_workers
        .get(&instance_id)
        .and_then(|worker_ids| worker_ids.first())
        .expect("event should bind a worker");

    let fx_state = backend
        .sampler_pool
        .fx_chain(worker_id)
        .expect("worker should expose fx chain state");
    assert!(!fx_state.fx_chain.low_pass.enabled);

    let mut wet_slot = low_pass_slot.clone();
    wet_slot
        .low_pass_effect_mut()
        .expect("slot should be low-pass")
        .enabled = true;
    backend
        .set_bus_effect_slot(bus.id, wet_slot)
        .expect("wet low-pass should update");

    let fx_state = backend
        .sampler_pool
        .fx_chain(worker_id)
        .expect("worker should still expose fx chain state");
    assert!(fx_state.fx_chain.low_pass.enabled);
    assert!((fx_state.fx_chain.low_pass.cutoff_hz - 450.0).abs() <= 0.01);

    backend
        .set_bus_effect_slot(bus.id, low_pass_slot)
        .expect("dry low-pass should update");

    let fx_state = backend
        .sampler_pool
        .fx_chain(worker_id)
        .expect("worker should still expose fx chain state");
    assert!(!fx_state.fx_chain.low_pass.enabled);
}

#[test]
fn new_workers_pick_up_current_bus_low_pass_state() {
    let mut backend = FirewheelBackend::new(Default::default())
        .expect("firewheel backend should start for local regression test");

    let asset_id = Uuid::now_v7();
    backend
        .register_interleaved_f32_asset(asset_id, 1, 48_000, vec![0.0; 48_000])
        .expect("synthetic sample should register");

    let mut bus = Bus::new("sfx_underwater");
    let mut dry_slot = BusEffectSlot::low_pass(450.0);
    dry_slot
        .low_pass_effect_mut()
        .expect("slot should be low-pass")
        .enabled = false;
    bus.effect_slots.push(dry_slot.clone());

    let event_id = EventId::new();
    let (sampler_id, sampler) = make_sampler(asset_id);
    let event = make_event(event_id, sampler_id, vec![sampler], bus.clone());

    let mut bank = Bank::new("core");
    bank.objects.events.push(event_id);
    bank.objects.buses.push(bus.id);

    backend
        .load_bank_with_definitions(
            bank,
            vec![event],
            vec![bus.clone()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("bank should load");

    let instance_a = backend.play(event_id).expect("first event should play");
    let worker_a = *backend
        .instance_workers
        .get(&instance_a)
        .and_then(|worker_ids| worker_ids.first())
        .expect("first event should bind a worker");
    assert!(
        !backend
            .sampler_pool
            .fx_chain(worker_a)
            .expect("first worker should expose fx chain state")
            .fx_chain
            .low_pass
            .enabled
    );

    let mut wet_slot = dry_slot.clone();
    wet_slot
        .low_pass_effect_mut()
        .expect("slot should be low-pass")
        .enabled = true;
    backend
        .set_bus_effect_slot(bus.id, wet_slot)
        .expect("wet low-pass should update");

    let instance_b = backend.play(event_id).expect("second event should play");
    let worker_b = *backend
        .instance_workers
        .get(&instance_b)
        .and_then(|worker_ids| worker_ids.first())
        .expect("second event should bind a worker");
    assert!(
        backend
            .sampler_pool
            .fx_chain(worker_b)
            .expect("second worker should expose fx chain state")
            .fx_chain
            .low_pass
            .enabled
    );

    backend
        .set_bus_effect_slot(bus.id, dry_slot)
        .expect("dry low-pass should update");

    let instance_c = backend.play(event_id).expect("third event should play");
    let worker_c = *backend
        .instance_workers
        .get(&instance_c)
        .and_then(|worker_ids| worker_ids.first())
        .expect("third event should bind a worker");
    assert!(
        !backend
            .sampler_pool
            .fx_chain(worker_c)
            .expect("third worker should expose fx chain state")
            .fx_chain
            .low_pass
            .enabled
    );
}
