//! Bevy 集成层骨架

use sonara_model::{Bank, BankId, Event, EventId, ParameterId, ParameterValue, SnapshotId};
use sonara_runtime::{
    EmitterId, EventInstanceId, Fade, PlaybackPlan, RuntimeError, SnapshotInstanceId, SonaraRuntime,
};

/// Sonara 的 Bevy 插件入口
#[derive(Debug, Default)]
pub struct SonaraPlugin;

/// Bevy 侧的全局音频入口
#[derive(Debug, Default)]
pub struct SonaraAudio {
    runtime: SonaraRuntime,
}

impl SonaraAudio {
    /// 创建一个新的音频入口
    pub fn new() -> Self {
        Self {
            runtime: SonaraRuntime::new(),
        }
    }

    /// 加载一个 bank
    ///
    /// 这里先保留最小骨架, 后续会改成真正的加载流程
    pub fn load_bank(&mut self, bank: Bank, events: Vec<Event>) -> Result<BankId, RuntimeError> {
        self.runtime.load_bank(bank, events)
    }

    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, RuntimeError> {
        self.runtime.play(event_id)
    }

    /// 创建一个 emitter
    pub fn create_emitter(&mut self) -> EmitterId {
        self.runtime.create_emitter()
    }

    /// 确保一个 AudioEmitter 已绑定到底层 runtime emitter
    ///
    /// 这模拟了 Bevy 侧组件第一次进入世界时的绑定过程。
    pub fn ensure_emitter(&mut self, emitter: &mut AudioEmitter) -> EmitterId {
        if let Some(id) = emitter.id {
            id
        } else {
            let id = self.runtime.create_emitter();
            emitter.id = Some(id);
            id
        }
    }

    /// 释放一个 AudioEmitter 已绑定的 runtime emitter
    pub fn detach_emitter(&mut self, emitter: &mut AudioEmitter) -> Result<(), RuntimeError> {
        if let Some(id) = emitter.id.take() {
            self.runtime.remove_emitter(id)?;
        }

        Ok(())
    }

    /// 在 emitter 上播放一个事件
    pub fn play_on(
        &mut self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<EventInstanceId, RuntimeError> {
        self.runtime.play_on(emitter_id, event_id)
    }

    /// 通过 AudioEmitter 组件播放事件
    pub fn play_from_emitter(
        &mut self,
        emitter: &mut AudioEmitter,
        event_id: EventId,
    ) -> Result<EventInstanceId, RuntimeError> {
        let emitter_id = self.ensure_emitter(emitter);
        self.runtime.play_on(emitter_id, event_id)
    }

    /// 设置一个全局参数
    pub fn set_global_param(
        &mut self,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        self.runtime.set_global_param(parameter_id, value)
    }

    /// 设置 emitter 参数
    pub fn set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        self.runtime
            .set_emitter_param(emitter_id, parameter_id, value)
    }

    /// 通过 AudioEmitter 组件设置 emitter 参数
    pub fn set_emitter_param_on(
        &mut self,
        emitter: &mut AudioEmitter,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        let emitter_id = self.ensure_emitter(emitter);
        self.runtime
            .set_emitter_param(emitter_id, parameter_id, value)
    }

    /// 读取一个事件实例当前解析出的播放计划
    pub fn active_plan(&self, instance_id: EventInstanceId) -> Option<&PlaybackPlan> {
        self.runtime.active_plan(instance_id)
    }

    /// 停止一个事件实例
    pub fn stop(&mut self, instance_id: EventInstanceId, fade: Fade) -> Result<(), RuntimeError> {
        self.runtime.stop(instance_id, fade)
    }

    /// 压入一个 snapshot
    pub fn push_snapshot(
        &mut self,
        snapshot_id: SnapshotId,
        fade: Fade,
    ) -> Result<SnapshotInstanceId, RuntimeError> {
        self.runtime.push_snapshot(snapshot_id, fade)
    }
}

/// 绑定到实体上的发声体组件
#[derive(Debug, Default)]
pub struct AudioEmitter {
    pub enabled: bool,
    pub id: Option<EmitterId>,
}

/// 绑定到实体上的监听器组件
#[derive(Debug, Default)]
pub struct AudioListener {
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use sonara_model::{
        EventContentRoot, EventKind, NodeId, NodeRef, SamplerNode, SpatialMode, SwitchCase,
        SwitchNode,
    };
    use uuid::Uuid;

    use super::*;

    fn make_switch_event(event_id: EventId, parameter_id: ParameterId, asset_id: Uuid) -> Event {
        let switch_id = NodeId::new();
        let sampler_id = NodeId::new();

        Event {
            id: event_id,
            name: "player.footstep".into(),
            kind: EventKind::OneShot,
            root: EventContentRoot {
                root: NodeRef { id: switch_id },
                nodes: vec![
                    sonara_model::EventContentNode::Switch(SwitchNode {
                        id: switch_id,
                        parameter_id,
                        cases: vec![SwitchCase {
                            variant: "stone".into(),
                            child: NodeRef { id: sampler_id },
                        }],
                        default_case: Some(NodeRef { id: sampler_id }),
                    }),
                    sonara_model::EventContentNode::Sampler(SamplerNode {
                        id: sampler_id,
                        asset_id,
                    }),
                ],
            },
            default_bus: None,
            spatial: SpatialMode::ThreeD,
            default_parameters: Vec::new(),
            voice_limit: None,
            steal_policy: None,
        }
    }

    #[test]
    fn ensure_emitter_reuses_existing_id() {
        let mut audio = SonaraAudio::new();
        let mut emitter = AudioEmitter::default();

        let first = audio.ensure_emitter(&mut emitter);
        let second = audio.ensure_emitter(&mut emitter);

        assert_eq!(Some(first), emitter.id);
        assert_eq!(first, second);
    }

    #[test]
    fn detach_emitter_clears_bound_id() {
        let mut audio = SonaraAudio::new();
        let mut emitter = AudioEmitter::default();

        let _ = audio.ensure_emitter(&mut emitter);
        audio
            .detach_emitter(&mut emitter)
            .expect("detach should succeed");

        assert_eq!(None, emitter.id);
    }

    #[test]
    fn play_from_emitter_uses_component_bound_emitter() {
        let surface_id = ParameterId::new();
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let event = make_switch_event(event_id, surface_id, asset_id);
        let mut bank = Bank::new("core");
        bank.events.push(event_id);

        let mut audio = SonaraAudio::new();
        audio
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let mut emitter = AudioEmitter::default();
        audio
            .set_emitter_param_on(
                &mut emitter,
                surface_id,
                ParameterValue::Enum("stone".into()),
            )
            .expect("emitter param should set");

        let instance_id = audio
            .play_from_emitter(&mut emitter, event_id)
            .expect("play should succeed");
        let plan = audio.active_plan(instance_id).expect("plan should exist");

        assert_eq!(plan.emitter_id, emitter.id);
        assert_eq!(plan.asset_ids, vec![asset_id]);
    }
}
