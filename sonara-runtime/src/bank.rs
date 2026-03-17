// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use sonara_model::{
    Bank, BankId, BankObjects, Bus, BusEffectSlot, BusId, Clip, ClipId, Event, EventId, MusicGraph,
    MusicGraphId, ParameterId, ParameterValue, ResumeSlot, ResumeSlotId, Snapshot, SnapshotId,
    SyncDomain, SyncDomainId,
};

use crate::error::RuntimeError;
use crate::ids::{EmitterId, EventInstanceId, MusicSessionId, SnapshotInstanceId};
use crate::types::{
    ActiveEventInstance, ActiveMusicSession, ActiveSnapshotInstance, ResumeMemoryEntry,
};

/// 面向游戏逻辑的运行时入口
#[derive(Debug, Default)]
pub struct SonaraRuntime {
    pub(crate) banks: HashMap<BankId, BankObjects>,
    pub(crate) buses: HashMap<BusId, Bus>,
    pub(crate) events: HashMap<EventId, Event>,
    pub(crate) clips: HashMap<ClipId, Clip>,
    pub(crate) resume_slots: HashMap<ResumeSlotId, ResumeSlot>,
    pub(crate) sync_domains: HashMap<SyncDomainId, SyncDomain>,
    pub(crate) music_graphs: HashMap<MusicGraphId, MusicGraph>,
    pub(crate) snapshots: HashMap<SnapshotId, Snapshot>,
    pub(crate) bus_volumes: HashMap<BusId, f32>,
    pub(crate) bus_effect_slots: HashMap<BusId, Vec<BusEffectSlot>>,
    pub(crate) global_parameters: HashMap<ParameterId, ParameterValue>,
    pub(crate) emitter_parameters: HashMap<EmitterId, HashMap<ParameterId, ParameterValue>>,
    pub(crate) active_instances: HashMap<EventInstanceId, ActiveEventInstance>,
    pub(crate) music_sessions: HashMap<MusicSessionId, ActiveMusicSession>,
    pub(crate) resume_memories: HashMap<ResumeSlotId, ResumeMemoryEntry>,
    pub(crate) active_snapshots: HashMap<SnapshotInstanceId, ActiveSnapshotInstance>,
    pub(crate) next_event_instance_id: u64,
    pub(crate) next_music_session_id: u64,
    pub(crate) next_snapshot_instance_id: u64,
    pub(crate) next_emitter_id: u64,
}

impl SonaraRuntime {
    /// 创建一个空运行时
    pub fn new() -> Self {
        Self::default()
    }

    /// 加载一个 bank 和它包含的事件定义
    pub fn load_bank(&mut self, bank: Bank, events: Vec<Event>) -> Result<BankId, RuntimeError> {
        self.load_bank_with_definitions(
            bank,
            events,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    /// 加载一个 bank 以及和它配套的高层对象定义。
    pub fn load_bank_with_definitions(
        &mut self,
        bank: Bank,
        events: Vec<Event>,
        buses: Vec<Bus>,
        snapshots: Vec<Snapshot>,
        clips: Vec<Clip>,
        resume_slots: Vec<ResumeSlot>,
        sync_domains: Vec<SyncDomain>,
        music_graphs: Vec<MusicGraph>,
    ) -> Result<BankId, RuntimeError> {
        let bank_id = bank.id;
        let bank_objects = bank.objects;

        for event in events {
            self.events.insert(event.id, event);
        }

        for bus in buses {
            self.buses.insert(bus.id, bus.clone());
            self.bus_volumes.entry(bus.id).or_insert(bus.default_volume);
            self.bus_effect_slots
                .insert(bus.id, bus.effect_slots.clone());
        }

        for bus_id in &bank_objects.buses {
            self.bus_volumes.entry(*bus_id).or_insert(1.0);
            self.bus_effect_slots.entry(*bus_id).or_default();
        }

        for snapshot in snapshots {
            self.snapshots.insert(snapshot.id, snapshot);
        }

        for clip in clips {
            self.clips.insert(clip.id, clip);
        }

        for resume_slot in resume_slots {
            self.resume_slots.insert(resume_slot.id, resume_slot);
        }

        for sync_domain in sync_domains {
            self.sync_domains.insert(sync_domain.id, sync_domain);
        }

        for music_graph in music_graphs {
            self.music_graphs.insert(music_graph.id, music_graph);
        }

        self.banks.insert(bank_id, bank_objects);

        Ok(bank_id)
    }

    /// 卸载一个 bank
    pub fn unload_bank(&mut self, bank_id: BankId) -> Result<(), RuntimeError> {
        let bank = self
            .banks
            .remove(&bank_id)
            .ok_or(RuntimeError::BankNotLoaded(bank_id))?;
        let event_ids = bank.events.clone();

        for event_id in &event_ids {
            self.events.remove(event_id);
        }

        for clip_id in &bank.clips {
            self.clips.remove(clip_id);
        }

        for resume_slot_id in &bank.resume_slots {
            self.resume_slots.remove(resume_slot_id);
            self.resume_memories.remove(resume_slot_id);
        }

        for sync_domain_id in &bank.sync_domains {
            self.sync_domains.remove(sync_domain_id);
        }

        for music_graph_id in &bank.music_graphs {
            self.music_graphs.remove(music_graph_id);
        }

        self.active_instances
            .retain(|_, instance| !event_ids.contains(&instance.event_id));
        self.music_sessions
            .retain(|_, session| !bank.music_graphs.contains(&session.graph_id));

        Ok(())
    }

    /// 判断某个 bank 是否已加载
    pub fn is_bank_loaded(&self, bank_id: BankId) -> bool {
        self.banks.contains_key(&bank_id)
    }

    /// 读取某个已加载 bank 的对象清单。
    pub fn loaded_bank_objects(&self, bank_id: BankId) -> Option<&BankObjects> {
        self.banks.get(&bank_id)
    }

    /// 读取一个已加载的 clip 定义。
    pub fn clip(&self, clip_id: ClipId) -> Option<&Clip> {
        self.clips.get(&clip_id)
    }

    /// 读取一个已加载的 bus 定义。
    pub fn bus(&self, bus_id: BusId) -> Option<&Bus> {
        self.buses.get(&bus_id)
    }

    /// 读取一个已加载的记忆槽定义。
    pub fn resume_slot(&self, resume_slot_id: ResumeSlotId) -> Option<&ResumeSlot> {
        self.resume_slots.get(&resume_slot_id)
    }

    /// 读取一个已加载的同步域定义。
    pub fn sync_domain(&self, sync_domain_id: SyncDomainId) -> Option<&SyncDomain> {
        self.sync_domains.get(&sync_domain_id)
    }

    /// 读取一个已加载的音乐图定义。
    pub fn music_graph(&self, music_graph_id: MusicGraphId) -> Option<&MusicGraph> {
        self.music_graphs.get(&music_graph_id)
    }

    /// 读取一个运行中的音乐会话。
    pub fn music_session(&self, session_id: MusicSessionId) -> Option<&ActiveMusicSession> {
        self.music_sessions.get(&session_id)
    }

    /// 读取一个记忆槽当前保存的播放头。
    pub fn resume_memory(&self, resume_slot_id: ResumeSlotId) -> Option<&ResumeMemoryEntry> {
        self.resume_memories.get(&resume_slot_id)
    }
}
