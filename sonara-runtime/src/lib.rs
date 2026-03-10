//! Sonara 的高层运行时接口

use std::collections::HashMap;

use sonara_model::{
    Bank, BankId, Event, EventContentNode, EventId, NodeId, NodeRef, ParameterId, ParameterValue,
    SnapshotId,
};
use thiserror::Error;
use uuid::Uuid;

/// 运行时事件实例 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventInstanceId(u64);

/// 运行时 snapshot 实例 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotInstanceId(u64);

/// 停止或切换时使用的淡变参数
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fade {
    pub duration_seconds: f32,
}

impl Fade {
    /// 立即切换, 不做淡变
    pub const IMMEDIATE: Self = Self {
        duration_seconds: 0.0,
    };

    /// 使用秒数构造淡变
    pub fn seconds(duration_seconds: f32) -> Self {
        Self { duration_seconds }
    }
}

/// 一次事件触发后得到的最小播放计划
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackPlan {
    pub event_id: EventId,
    pub asset_ids: Vec<Uuid>,
}

/// 运行中的事件实例
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEventInstance {
    pub id: EventInstanceId,
    pub event_id: EventId,
    pub plan: PlaybackPlan,
}

/// 运行时错误
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("event `{0:?}` is not loaded")]
    EventNotLoaded(EventId),
    #[error("bank `{0:?}` is not loaded")]
    BankNotLoaded(BankId),
    #[error("parameter `{0:?}` is not available")]
    ParameterUnavailable(ParameterId),
    #[error("switch parameter `{0:?}` is not an enum value")]
    SwitchParameterTypeMismatch(ParameterId),
    #[error("switch parameter `{0:?}` 没有匹配分支")]
    NoMatchingSwitchCase(ParameterId),
    #[error("事件内容树中缺少节点 `{0:?}`")]
    MissingNode(NodeId),
    #[error("事件实例 `{0:?}` 不存在")]
    EventInstanceNotFound(EventInstanceId),
}

/// 面向游戏逻辑的运行时入口
#[derive(Debug, Default)]
pub struct SonaraRuntime {
    banks: HashMap<BankId, Bank>,
    events: HashMap<EventId, Event>,
    global_parameters: HashMap<ParameterId, ParameterValue>,
    active_instances: HashMap<EventInstanceId, ActiveEventInstance>,
    next_event_instance_id: u64,
}

impl SonaraRuntime {
    /// 创建一个空运行时
    pub fn new() -> Self {
        Self::default()
    }

    /// 加载一个 bank 和它包含的事件定义
    pub fn load_bank(&mut self, bank: Bank, events: Vec<Event>) -> Result<BankId, RuntimeError> {
        let bank_id = bank.id;

        for event in events {
            self.events.insert(event.id, event);
        }

        self.banks.insert(bank_id, bank);

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
            self.events.remove(&event_id);
        }

        self.active_instances
            .retain(|_, instance| !event_ids.contains(&instance.event_id));

        Ok(())
    }

    /// 判断某个 bank 是否已加载
    pub fn is_bank_loaded(&self, bank_id: BankId) -> bool {
        self.banks.contains_key(&bank_id)
    }

    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, RuntimeError> {
        let plan = self.plan_event(event_id)?;
        let instance_id = EventInstanceId(self.next_event_instance_id);
        self.next_event_instance_id += 1;

        self.active_instances.insert(
            instance_id,
            ActiveEventInstance {
                id: instance_id,
                event_id,
                plan,
            },
        );

        Ok(instance_id)
    }

    /// 在不创建实例的情况下解析一个事件
    pub fn plan_event(&self, event_id: EventId) -> Result<PlaybackPlan, RuntimeError> {
        let event = self
            .events
            .get(&event_id)
            .ok_or(RuntimeError::EventNotLoaded(event_id))?;

        let node_lookup: HashMap<NodeId, &EventContentNode> = event
            .root
            .nodes
            .iter()
            .map(|node| (node.id(), node))
            .collect();
        let mut asset_ids = Vec::new();

        self.resolve_node(&node_lookup, event.root.root, &mut asset_ids)?;

        Ok(PlaybackPlan {
            event_id,
            asset_ids,
        })
    }

    /// 停止一个事件实例
    pub fn stop(&mut self, instance_id: EventInstanceId, _fade: Fade) -> Result<(), RuntimeError> {
        self.active_instances
            .remove(&instance_id)
            .map(|_| ())
            .ok_or(RuntimeError::EventInstanceNotFound(instance_id))
    }

    /// 获取事件实例的当前播放计划
    pub fn active_plan(&self, instance_id: EventInstanceId) -> Option<&PlaybackPlan> {
        self.active_instances
            .get(&instance_id)
            .map(|instance| &instance.plan)
    }

    /// 设置全局参数
    pub fn set_global_param(
        &mut self,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        self.global_parameters.insert(parameter_id, value);
        Ok(())
    }

    /// 读取一个全局参数
    pub fn global_param(&self, parameter_id: ParameterId) -> Option<&ParameterValue> {
        self.global_parameters.get(&parameter_id)
    }

    /// 压入一个 snapshot
    pub fn push_snapshot(
        &mut self,
        _snapshot_id: SnapshotId,
        _fade: Fade,
    ) -> Result<SnapshotInstanceId, RuntimeError> {
        Ok(SnapshotInstanceId(0))
    }

    fn resolve_node(
        &self,
        node_lookup: &HashMap<NodeId, &EventContentNode>,
        node_ref: NodeRef,
        asset_ids: &mut Vec<Uuid>,
    ) -> Result<(), RuntimeError> {
        let node = node_lookup
            .get(&node_ref.id)
            .ok_or(RuntimeError::MissingNode(node_ref.id))?;

        match node {
            EventContentNode::Sampler(node) => {
                asset_ids.push(node.asset_id);
            }
            EventContentNode::Random(node) => {
                // v0 先固定选择第一个分支, 让规划结果可预测且方便测试
                if let Some(child) = node.children.first().copied() {
                    self.resolve_node(node_lookup, child, asset_ids)?;
                }
            }
            EventContentNode::Sequence(node) | EventContentNode::Layer(node) => {
                for child in &node.children {
                    self.resolve_node(node_lookup, *child, asset_ids)?;
                }
            }
            EventContentNode::Switch(node) => {
                let selected = match self.global_parameters.get(&node.parameter_id) {
                    Some(ParameterValue::Enum(variant)) => node
                        .cases
                        .iter()
                        .find(|case| case.variant == *variant)
                        .map(|case| case.child)
                        .or(node.default_case),
                    Some(_) => {
                        return Err(RuntimeError::SwitchParameterTypeMismatch(node.parameter_id));
                    }
                    None => node.default_case,
                }
                .ok_or(RuntimeError::NoMatchingSwitchCase(node.parameter_id))?;

                self.resolve_node(node_lookup, selected, asset_ids)?;
            }
            EventContentNode::Loop(node) => {
                // v0 只为 loop 规划一次内容
                self.resolve_node(node_lookup, node.child, asset_ids)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use sonara_model::{
        EventContentRoot, EventKind, SamplerNode, SequenceNode, SpatialMode, SwitchCase, SwitchNode,
    };

    use super::*;

    fn make_sampler(asset_id: Uuid) -> (NodeId, EventContentNode) {
        let id = NodeId::new();
        (id, EventContentNode::Sampler(SamplerNode { id, asset_id }))
    }

    fn make_event(id: EventId, root: NodeId, nodes: Vec<EventContentNode>) -> Event {
        Event {
            id,
            name: SmolStr::new("player.footstep"),
            kind: EventKind::OneShot,
            root: EventContentRoot {
                root: NodeRef { id: root },
                nodes,
            },
            default_bus: None,
            spatial: SpatialMode::ThreeD,
            default_parameters: Vec::new(),
            voice_limit: None,
            steal_policy: None,
        }
    }

    #[test]
    fn play_creates_an_active_instance_with_plan() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let (sampler_id, sampler) = make_sampler(asset_id);
        let event = make_event(event_id, sampler_id, vec![sampler]);
        let mut bank = Bank::new("core");
        bank.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let instance_id = runtime.play(event_id).expect("event should play");

        assert_eq!(
            runtime.active_plan(instance_id),
            Some(&PlaybackPlan {
                event_id,
                asset_ids: vec![asset_id],
            })
        );
    }

    #[test]
    fn plan_event_resolves_switch_from_global_param() {
        let event_id = EventId::new();
        let surface_id = ParameterId::new();
        let switch_id = NodeId::new();
        let wood_asset = Uuid::now_v7();
        let stone_asset = Uuid::now_v7();
        let (wood_node_id, wood_sampler) = make_sampler(wood_asset);
        let (stone_node_id, stone_sampler) = make_sampler(stone_asset);

        let event = make_event(
            event_id,
            switch_id,
            vec![
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
                wood_sampler,
                stone_sampler,
            ],
        );

        let mut bank = Bank::new("core");
        bank.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");
        runtime
            .set_global_param(surface_id, ParameterValue::Enum("stone".into()))
            .expect("param should set");

        let plan = runtime.plan_event(event_id).expect("plan should resolve");

        assert_eq!(plan.asset_ids, vec![stone_asset]);
    }

    #[test]
    fn plan_event_resolves_sequence_children_in_order() {
        let event_id = EventId::new();
        let root_id = NodeId::new();
        let asset_a = Uuid::now_v7();
        let asset_b = Uuid::now_v7();
        let (node_a, sampler_a) = make_sampler(asset_a);
        let (node_b, sampler_b) = make_sampler(asset_b);

        let event = make_event(
            event_id,
            root_id,
            vec![
                EventContentNode::Sequence(SequenceNode {
                    id: root_id,
                    children: vec![NodeRef { id: node_a }, NodeRef { id: node_b }],
                }),
                sampler_a,
                sampler_b,
            ],
        );

        let mut bank = Bank::new("core");
        bank.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let plan = runtime.plan_event(event_id).expect("plan should resolve");

        assert_eq!(plan.asset_ids, vec![asset_a, asset_b]);
    }
}
