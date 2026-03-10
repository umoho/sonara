use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use crate::ids::{BusId, EventId, ParameterId};

/// 面向游戏逻辑的主音频行为定义
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub name: SmolStr,
    pub kind: EventKind,
    pub root: EventContentRoot,
    pub default_bus: Option<BusId>,
    pub spatial: SpatialMode,
    pub default_parameters: Vec<ParameterId>,
    pub voice_limit: Option<u16>,
    pub steal_policy: Option<SmolStr>,
}

/// 事件生命周期分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    /// 短生命周期事件, 例如脚步和爆炸
    OneShot,
    /// 长生命周期事件, 例如环境声和音乐
    Persistent,
}

/// 事件默认的空间化模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpatialMode {
    None,
    TwoD,
    ThreeD,
}

/// 事件内容树的根
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventContentRoot {
    pub root: NodeRef,
    pub nodes: Vec<EventContentNode>,
}

/// 内容树节点 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    /// 生成一个新的节点 ID
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// 节点引用
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeRef {
    pub id: NodeId,
}

/// 事件内容树中的节点
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventContentNode {
    Sampler(SamplerNode),
    Random(RandomNode),
    Sequence(SequenceNode),
    Layer(SequenceNode),
    Switch(SwitchNode),
    Loop(LoopNode),
}

impl EventContentNode {
    /// 获取节点 ID
    pub fn id(&self) -> NodeId {
        match self {
            Self::Sampler(node) => node.id,
            Self::Random(node) => node.id,
            Self::Sequence(node) => node.id,
            Self::Layer(node) => node.id,
            Self::Switch(node) => node.id,
            Self::Loop(node) => node.id,
        }
    }
}

/// 叶子节点, 直接引用一个音频资源
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamplerNode {
    pub id: NodeId,
    pub asset_id: Uuid,
}

pub type LeafNode = SamplerNode;

/// 随机容器
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RandomNode {
    pub id: NodeId,
    pub children: Vec<NodeRef>,
}

/// 顺序容器
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceNode {
    pub id: NodeId,
    pub children: Vec<NodeRef>,
}

/// 按枚举参数切换分支的容器
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchNode {
    pub id: NodeId,
    pub parameter_id: ParameterId,
    pub cases: Vec<SwitchCase>,
    pub default_case: Option<NodeRef>,
}

/// Switch 中的单个分支
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchCase {
    pub variant: SmolStr,
    pub child: NodeRef,
}

/// 循环容器
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoopNode {
    pub id: NodeId,
    pub child: NodeRef,
}
