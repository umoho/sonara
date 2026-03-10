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

/// 运行时 emitter ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EmitterId(u64);

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
    pub emitter_id: Option<EmitterId>,
    pub asset_ids: Vec<Uuid>,
}

/// 运行中的事件实例
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEventInstance {
    pub id: EventInstanceId,
    pub event_id: EventId,
    pub emitter_id: Option<EmitterId>,
    pub plan: PlaybackPlan,
}

/// 运行时可消费的一条最小请求
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeRequest {
    Play {
        event_id: EventId,
    },
    PlayOnEmitter {
        emitter_id: EmitterId,
        event_id: EventId,
    },
    SetGlobalParam {
        parameter_id: ParameterId,
        value: ParameterValue,
    },
    SetEmitterParam {
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    },
    Stop {
        instance_id: EventInstanceId,
        fade: Fade,
    },
}

/// 运行时执行请求后的结果
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeRequestResult {
    Played { instance_id: EventInstanceId },
    Stopped { instance_id: EventInstanceId },
    ParameterSet,
}

/// 默认使用的运行时命令缓冲区类型
pub type RuntimeCommandBuffer = AudioCommandBuffer<RuntimeRequest>;

/// 一组待执行的音频请求缓冲区
#[derive(Debug)]
pub struct AudioCommandBuffer<Request> {
    requests: Vec<Request>,
}

impl<Request> Default for AudioCommandBuffer<Request> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Request> AudioCommandBuffer<Request> {
    /// 创建一个空缓冲区
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
        }
    }

    /// 追加一条请求
    pub fn push(&mut self, request: Request) {
        self.requests.push(request);
    }

    /// 取出当前所有待处理请求
    pub fn drain(&mut self) -> Vec<Request> {
        self.requests.drain(..).collect()
    }

    /// 当前缓冲区里的请求数量
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// 当前缓冲区是否为空
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// 依次执行所有待处理请求, 遇到第一条错误立即返回
    pub fn apply<Output, Error, Executor>(
        &mut self,
        mut executor: Executor,
    ) -> Result<Vec<Output>, Error>
    where
        Executor: FnMut(&Request) -> Result<Output, Error>,
    {
        let requests = self.drain();
        let mut results = Vec::with_capacity(requests.len());

        for request in requests {
            results.push(executor(&request)?);
        }

        Ok(results)
    }

    /// 依次执行所有待处理请求, 单条失败不会中断整批处理
    pub fn apply_isolated<Output, Error, Executor>(
        &mut self,
        mut executor: Executor,
    ) -> Vec<AudioCommandOutcome<Request, Output, Error>>
    where
        Executor: FnMut(&Request) -> Result<Output, Error>,
    {
        self.drain()
            .into_iter()
            .map(|request| {
                let result = executor(&request);
                AudioCommandOutcome { request, result }
            })
            .collect()
    }
}

impl RuntimeRequest {
    /// 构造一个未绑定 emitter 的播放请求
    pub fn play(event_id: EventId) -> Self {
        Self::Play { event_id }
    }

    /// 构造一个面向指定 emitter 的播放请求
    pub fn play_on(emitter_id: EmitterId, event_id: EventId) -> Self {
        Self::PlayOnEmitter {
            emitter_id,
            event_id,
        }
    }

    /// 构造一个全局参数更新请求
    pub fn set_global_param(parameter_id: ParameterId, value: ParameterValue) -> Self {
        Self::SetGlobalParam {
            parameter_id,
            value,
        }
    }

    /// 构造一个 emitter 参数更新请求
    pub fn set_emitter_param(
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Self {
        Self::SetEmitterParam {
            emitter_id,
            parameter_id,
            value,
        }
    }

    /// 构造一个停止实例请求
    pub fn stop(instance_id: EventInstanceId, fade: Fade) -> Self {
        Self::Stop { instance_id, fade }
    }
}

impl AudioCommandBuffer<RuntimeRequest> {
    /// 排队一个未绑定 emitter 的播放请求
    pub fn queue_play(&mut self, event_id: EventId) {
        self.push(RuntimeRequest::play(event_id));
    }

    /// 排队一个面向指定 emitter 的播放请求
    pub fn queue_play_on(&mut self, emitter_id: EmitterId, event_id: EventId) {
        self.push(RuntimeRequest::play_on(emitter_id, event_id));
    }

    /// 排队一个全局参数更新请求
    pub fn queue_set_global_param(&mut self, parameter_id: ParameterId, value: ParameterValue) {
        self.push(RuntimeRequest::set_global_param(parameter_id, value));
    }

    /// 排队一个 emitter 参数更新请求
    pub fn queue_set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) {
        self.push(RuntimeRequest::set_emitter_param(
            emitter_id,
            parameter_id,
            value,
        ));
    }

    /// 排队一个停止实例请求
    pub fn queue_stop(&mut self, instance_id: EventInstanceId, fade: Fade) {
        self.push(RuntimeRequest::stop(instance_id, fade));
    }
}

/// 一条请求在隔离执行模式下的结果
#[derive(Debug)]
pub struct AudioCommandOutcome<Request, Output, Error> {
    pub request: Request,
    pub result: Result<Output, Error>,
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
    #[error("emitter `{0:?}` 不存在")]
    EmitterNotFound(EmitterId),
}

/// 面向游戏逻辑的运行时入口
#[derive(Debug, Default)]
pub struct SonaraRuntime {
    banks: HashMap<BankId, Bank>,
    events: HashMap<EventId, Event>,
    global_parameters: HashMap<ParameterId, ParameterValue>,
    emitter_parameters: HashMap<EmitterId, HashMap<ParameterId, ParameterValue>>,
    active_instances: HashMap<EventInstanceId, ActiveEventInstance>,
    next_event_instance_id: u64,
    next_emitter_id: u64,
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
        let event_ids = bank.objects.events.clone();

        for event_id in &event_ids {
            self.events.remove(event_id);
        }

        self.active_instances
            .retain(|_, instance| !event_ids.contains(&instance.event_id));

        Ok(())
    }

    /// 判断某个 bank 是否已加载
    pub fn is_bank_loaded(&self, bank_id: BankId) -> bool {
        self.banks.contains_key(&bank_id)
    }

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

    /// 压入一个 snapshot
    pub fn push_snapshot(
        &mut self,
        _snapshot_id: SnapshotId,
        _fade: Fade,
    ) -> Result<SnapshotInstanceId, RuntimeError> {
        Ok(SnapshotInstanceId(0))
    }

    /// 执行一条最小运行时请求
    pub fn apply_request(
        &mut self,
        request: &RuntimeRequest,
    ) -> Result<RuntimeRequestResult, RuntimeError> {
        match request {
            RuntimeRequest::Play { event_id } => Ok(RuntimeRequestResult::Played {
                instance_id: self.play(*event_id)?,
            }),
            RuntimeRequest::PlayOnEmitter {
                emitter_id,
                event_id,
            } => Ok(RuntimeRequestResult::Played {
                instance_id: self.play_on(*emitter_id, *event_id)?,
            }),
            RuntimeRequest::SetGlobalParam {
                parameter_id,
                value,
            } => {
                self.set_global_param(*parameter_id, value.clone())?;
                Ok(RuntimeRequestResult::ParameterSet)
            }
            RuntimeRequest::SetEmitterParam {
                emitter_id,
                parameter_id,
                value,
            } => {
                self.set_emitter_param(*emitter_id, *parameter_id, value.clone())?;
                Ok(RuntimeRequestResult::ParameterSet)
            }
            RuntimeRequest::Stop { instance_id, fade } => {
                self.stop(*instance_id, *fade)?;
                Ok(RuntimeRequestResult::Stopped {
                    instance_id: *instance_id,
                })
            }
        }
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
        node: &sonara_model::SwitchNode,
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
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let instance_id = runtime.play(event_id).expect("event should play");

        assert_eq!(
            runtime.active_plan(instance_id),
            Some(&PlaybackPlan {
                event_id,
                emitter_id: None,
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
        bank.objects.events.push(event_id);

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
    fn plan_event_on_prefers_emitter_param_over_global_param() {
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
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");
        let emitter_id = runtime.create_emitter();
        runtime
            .set_global_param(surface_id, ParameterValue::Enum("wood".into()))
            .expect("param should set");
        runtime
            .set_emitter_param(emitter_id, surface_id, ParameterValue::Enum("stone".into()))
            .expect("emitter param should set");

        let plan = runtime
            .plan_event_on(emitter_id, event_id)
            .expect("plan should resolve");

        assert_eq!(plan.asset_ids, vec![stone_asset]);
        assert_eq!(plan.emitter_id, Some(emitter_id));
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
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let plan = runtime.plan_event(event_id).expect("plan should resolve");

        assert_eq!(plan.asset_ids, vec![asset_a, asset_b]);
    }

    #[test]
    fn audio_command_buffer_applies_requests_in_order() {
        let mut buffer = AudioCommandBuffer::new();
        buffer.push(1);
        buffer.push(2);

        let results = buffer
            .apply(|value| Ok::<_, ()>(value * 10))
            .expect("apply should succeed");

        assert_eq!(results, vec![10, 20]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn audio_command_buffer_isolates_per_request_failures() {
        let mut buffer = AudioCommandBuffer::new();
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        let outcomes = buffer.apply_isolated(|value| {
            if *value == 2 {
                Err("boom")
            } else {
                Ok(value * 10)
            }
        });

        assert_eq!(outcomes.len(), 3);
        assert!(matches!(outcomes[0].result, Ok(10)));
        assert!(matches!(outcomes[1].result, Err("boom")));
        assert!(matches!(outcomes[2].result, Ok(30)));
        assert!(buffer.is_empty());
    }

    #[test]
    fn stop_request_removes_active_instance() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let (sampler_id, sampler) = make_sampler(asset_id);
        let event = make_event(event_id, sampler_id, vec![sampler]);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let instance_id = runtime.play(event_id).expect("event should play");
        let result = runtime
            .apply_request(&RuntimeRequest::stop(instance_id, Fade::IMMEDIATE))
            .expect("stop should succeed");

        assert_eq!(result, RuntimeRequestResult::Stopped { instance_id });
        assert_eq!(runtime.active_plan(instance_id), None);
    }
}
