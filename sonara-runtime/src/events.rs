// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use sonara_model::{
    EventContentNode, EventId, NodeId, NodeRef, ParameterId, ParameterValue, SwitchNode,
};
use uuid::Uuid;

use crate::bank::SonaraRuntime;
use crate::error::RuntimeError;
use crate::ids::{EmitterId, EventInstanceId, SnapshotInstanceId};
use crate::types::{
    ActiveEventInstance, ActiveSnapshotInstance, EventInstanceState, Fade, PlaybackPlan,
};

impl SonaraRuntime {
    /// 创建一个新的 emitter
    pub fn create_emitter(&mut self) -> EmitterId {
        let emitter_id = EmitterId(self.next_emitter_id);
        self.next_emitter_id += 1;
        self.emitter_parameters.insert(emitter_id, HashMap::new());
        emitter_id
    }

    /// 删除一个 emitter
    pub fn remove_emitter(&mut self, emitter_id: EmitterId) -> Result<(), RuntimeError> {
        self.emitter_parameters
            .remove(&emitter_id)
            .map(|_| ())
            .ok_or(RuntimeError::EmitterNotFound(emitter_id))
    }

    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, RuntimeError> {
        self.play_internal(event_id, None)
    }

    /// 在 emitter 上播放一个事件
    pub fn play_on(
        &mut self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<EventInstanceId, RuntimeError> {
        if !self.emitter_parameters.contains_key(&emitter_id) {
            return Err(RuntimeError::EmitterNotFound(emitter_id));
        }

        self.play_internal(event_id, Some(emitter_id))
    }

    /// 在不创建实例的情况下解析一个事件
    pub fn plan_event(&self, event_id: EventId) -> Result<PlaybackPlan, RuntimeError> {
        self.plan_event_for_emitter(None, event_id)
    }

    /// 在指定 emitter 上解析一个事件
    pub fn plan_event_on(
        &self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<PlaybackPlan, RuntimeError> {
        if !self.emitter_parameters.contains_key(&emitter_id) {
            return Err(RuntimeError::EmitterNotFound(emitter_id));
        }

        self.plan_event_for_emitter(Some(emitter_id), event_id)
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

    /// 查询一个事件实例当前对游戏侧可见的播放状态。
    pub fn instance_state(&self, instance_id: EventInstanceId) -> EventInstanceState {
        if self.active_instances.contains_key(&instance_id) {
            EventInstanceState::Playing
        } else {
            EventInstanceState::Stopped
        }
    }

    /// 读取一个运行中的 snapshot 实例。
    pub fn active_snapshot(
        &self,
        instance_id: SnapshotInstanceId,
    ) -> Option<&ActiveSnapshotInstance> {
        self.active_snapshots.get(&instance_id)
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

    /// 设置 emitter 参数
    pub fn set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        let parameters = self
            .emitter_parameters
            .get_mut(&emitter_id)
            .ok_or(RuntimeError::EmitterNotFound(emitter_id))?;

        parameters.insert(parameter_id, value);
        Ok(())
    }

    /// 读取 emitter 参数
    pub fn emitter_param(
        &self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
    ) -> Option<&ParameterValue> {
        self.emitter_parameters
            .get(&emitter_id)
            .and_then(|parameters| parameters.get(&parameter_id))
    }

    fn play_internal(
        &mut self,
        event_id: EventId,
        emitter_id: Option<EmitterId>,
    ) -> Result<EventInstanceId, RuntimeError> {
        let plan = self.plan_event_for_emitter(emitter_id, event_id)?;
        let instance_id = EventInstanceId(self.next_event_instance_id);
        self.next_event_instance_id += 1;

        self.active_instances.insert(
            instance_id,
            ActiveEventInstance {
                id: instance_id,
                event_id,
                emitter_id,
                plan,
            },
        );

        Ok(instance_id)
    }

    fn plan_event_for_emitter(
        &self,
        emitter_id: Option<EmitterId>,
        event_id: EventId,
    ) -> Result<PlaybackPlan, RuntimeError> {
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

        self.resolve_node(&node_lookup, emitter_id, event.root.root, &mut asset_ids)?;

        Ok(PlaybackPlan {
            event_id,
            emitter_id,
            asset_ids,
        })
    }

    fn resolve_node(
        &self,
        node_lookup: &HashMap<NodeId, &EventContentNode>,
        emitter_id: Option<EmitterId>,
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
                    self.resolve_node(node_lookup, emitter_id, child, asset_ids)?;
                }
            }
            EventContentNode::Sequence(node) | EventContentNode::Layer(node) => {
                for child in &node.children {
                    self.resolve_node(node_lookup, emitter_id, *child, asset_ids)?;
                }
            }
            EventContentNode::Switch(node) => {
                let selected = self
                    .resolve_switch_target(emitter_id, node.parameter_id, node)
                    .and_then(|selected| {
                        selected.ok_or(RuntimeError::NoMatchingSwitchCase(node.parameter_id))
                    })?;

                self.resolve_node(node_lookup, emitter_id, selected, asset_ids)?;
            }
            EventContentNode::Loop(node) => {
                // v0 只为 loop 规划一次内容
                self.resolve_node(node_lookup, emitter_id, node.child, asset_ids)?;
            }
        }

        Ok(())
    }

    fn resolve_switch_target(
        &self,
        emitter_id: Option<EmitterId>,
        parameter_id: ParameterId,
        node: &SwitchNode,
    ) -> Result<Option<NodeRef>, RuntimeError> {
        let parameter_value = emitter_id
            .and_then(|emitter_id| self.emitter_param(emitter_id, parameter_id))
            .or_else(|| self.global_param(parameter_id));

        let selected = match parameter_value {
            Some(ParameterValue::Enum(variant)) => node
                .cases
                .iter()
                .find(|case| case.variant == *variant)
                .map(|case| case.child)
                .or(node.default_case),
            Some(_) => {
                return Err(RuntimeError::SwitchParameterTypeMismatch(parameter_id));
            }
            None => node.default_case,
        };

        Ok(selected)
    }
}
