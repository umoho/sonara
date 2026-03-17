// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use sonara_model::{BusEffectSlot, BusId, Snapshot, SnapshotId, TrackId};

use crate::bank::SonaraRuntime;
use crate::error::RuntimeError;
use crate::ids::{EventInstanceId, MusicSessionId, SnapshotInstanceId};
use crate::types::{ActiveSnapshotInstance, Fade};

impl SonaraRuntime {
    /// 加载一个 snapshot 定义。
    pub fn load_snapshot(&mut self, snapshot: Snapshot) {
        self.snapshots.insert(snapshot.id, snapshot);
    }

    /// 设置某个 bus 当前的 live gain。
    pub fn set_bus_gain(&mut self, bus_id: BusId, gain: f32) -> Result<(), RuntimeError> {
        let bus_gain = self
            .bus_volumes
            .get_mut(&bus_id)
            .ok_or(RuntimeError::BusNotLoaded(bus_id))?;
        *bus_gain = gain.max(0.0);
        Ok(())
    }

    /// 读取当前某个 bus 的 live gain。
    pub fn bus_gain(&self, bus_id: BusId) -> Option<f32> {
        self.bus_volumes.get(&bus_id).copied()
    }

    /// 读取当前某个 bus 的目标音量。
    pub fn bus_volume(&self, bus_id: BusId) -> Option<f32> {
        self.bus_gain(bus_id)
    }

    /// 读取某个 bus 当前的 live effect slot 列表。
    pub fn bus_effect_slots(&self, bus_id: BusId) -> Option<&[BusEffectSlot]> {
        self.bus_effect_slots.get(&bus_id).map(Vec::as_slice)
    }

    /// 替换某个 bus 上的一个 live effect slot。
    pub fn set_bus_effect_slot(
        &mut self,
        bus_id: BusId,
        slot: BusEffectSlot,
    ) -> Result<(), RuntimeError> {
        let slots = self
            .bus_effect_slots
            .get_mut(&bus_id)
            .ok_or(RuntimeError::BusNotLoaded(bus_id))?;
        let existing = slots
            .iter_mut()
            .find(|candidate| candidate.id == slot.id)
            .ok_or(RuntimeError::BusEffectSlotNotFound {
                bus_id,
                slot_id: slot.id,
            })?;
        *existing = slot;
        Ok(())
    }

    /// 读取某个事件实例当前命中的默认 bus。
    pub fn active_event_bus(&self, instance_id: EventInstanceId) -> Option<BusId> {
        let instance = self.active_instances.get(&instance_id)?;
        let event = self.events.get(&instance.event_id)?;
        event.default_bus
    }

    /// 读取某个事件实例当前命中的默认 bus 音量。
    ///
    /// 如果事件没有默认 bus，则返回 `1.0`。
    pub fn active_bus_gain(&self, instance_id: EventInstanceId) -> Option<f32> {
        Some(
            self.active_event_bus(instance_id)
                .and_then(|bus_id| self.bus_gain(bus_id))
                .unwrap_or(1.0),
        )
    }

    /// 读取某个事件实例当前命中的默认 bus 音量。
    ///
    /// 如果事件没有默认 bus，则返回 `1.0`。
    pub fn active_bus_volume(&self, instance_id: EventInstanceId) -> Option<f32> {
        self.active_bus_gain(instance_id)
    }

    /// 读取音乐会话中某个 track 当前声明的输出 bus。
    pub fn music_track_output_bus(
        &self,
        session_id: MusicSessionId,
        track_id: TrackId,
    ) -> Result<Option<BusId>, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;

        Ok(graph.track(track_id).and_then(|track| track.output_bus))
    }

    /// 压入一个 snapshot
    pub fn push_snapshot(
        &mut self,
        snapshot_id: SnapshotId,
        fade: Fade,
    ) -> Result<SnapshotInstanceId, RuntimeError> {
        let snapshot = self
            .snapshots
            .get(&snapshot_id)
            .ok_or(RuntimeError::SnapshotNotLoaded(snapshot_id))?
            .clone();
        let mut overrides = HashMap::with_capacity(snapshot.targets.len());

        for target in &snapshot.targets {
            if !self.bus_volumes.contains_key(&target.bus_id) {
                return Err(RuntimeError::SnapshotTargetBusNotFound(target.bus_id));
            }

            self.set_bus_gain(target.bus_id, target.target_volume)?;
            overrides.insert(target.bus_id, target.target_volume);
        }

        let instance_id = SnapshotInstanceId(self.next_snapshot_instance_id);
        self.next_snapshot_instance_id += 1;
        self.active_snapshots.insert(
            instance_id,
            ActiveSnapshotInstance {
                id: instance_id,
                snapshot_id,
                fade,
                overrides,
            },
        );

        Ok(instance_id)
    }
}
