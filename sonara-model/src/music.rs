use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::{ClipId, MusicGraphId, MusicNodeId, ResumeSlotId, TrackId};

fn default_true() -> bool {
    true
}

/// 一个音乐图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicGraph {
    pub id: MusicGraphId,
    pub name: SmolStr,
    #[serde(default)]
    pub initial_state: Option<MusicNodeId>,
    pub initial_node: Option<MusicNodeId>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub states: Vec<MusicNode>,
    #[serde(default)]
    pub nodes: Vec<MusicNode>,
    #[serde(default)]
    pub transitions: Vec<MusicEdge>,
    #[serde(default)]
    pub edges: Vec<MusicEdge>,
}

impl MusicGraph {
    /// 创建一个新的音乐图。
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            id: MusicGraphId::new(),
            name: name.into(),
            initial_state: None,
            initial_node: None,
            tracks: Vec::new(),
            states: Vec::new(),
            nodes: Vec::new(),
            transitions: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// 查找一个显式声明的 track。
    pub fn track(&self, track_id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|track| track.id == track_id)
    }

    /// 读取图中声明的主 track。
    pub fn main_track(&self) -> Option<&Track> {
        self.tracks
            .iter()
            .find(|track| matches!(track.role, TrackRole::Main))
    }

    /// 按角色读取图中声明的 track。
    pub fn track_by_role(&self, role: TrackRole) -> Option<&Track> {
        self.tracks.iter().find(|track| track.role == role)
    }

    /// 查找一个节点定义。
    pub fn node(&self, node_id: MusicNodeId) -> Option<&MusicNode> {
        self.all_nodes().iter().find(|node| node.id == node_id)
    }

    /// 读取图中的节点集合，优先使用新字段。
    pub fn all_nodes(&self) -> &[MusicNode] {
        if self.nodes.is_empty() {
            &self.states
        } else {
            &self.nodes
        }
    }

    /// 读取图中的边集合，优先使用新字段。
    pub fn all_edges(&self) -> &[MusicEdge] {
        if self.edges.is_empty() {
            &self.transitions
        } else {
            &self.edges
        }
    }

    /// 读取图中的初始节点，优先使用新字段。
    pub fn start_node(&self) -> Option<MusicNodeId> {
        self.initial_node.or(self.initial_state)
    }
}

/// 音乐图中的一个节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicNode {
    pub id: MusicNodeId,
    pub name: SmolStr,
    pub target: PlaybackTarget,
    #[serde(default)]
    pub bindings: Vec<TrackBinding>,
    pub memory_slot: Option<ResumeSlotId>,
    #[serde(default)]
    pub memory_policy: MemoryPolicy,
    #[serde(default)]
    pub default_entry: EntryPolicy,
    #[serde(default = "default_true")]
    pub externally_targetable: bool,
    #[serde(default)]
    pub completion_source: Option<TrackId>,
}

impl MusicNode {
    /// 读取当前节点绑定到指定 track 的目标。
    pub fn binding_for_track(&self, track_id: TrackId) -> Option<&TrackBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.track_id == track_id)
    }

    /// 按角色读取当前节点的 track 绑定。
    pub fn binding_for_role<'a>(
        &'a self,
        graph: &'a MusicGraph,
        role: TrackRole,
    ) -> Option<&'a TrackBinding> {
        graph
            .track_by_role(role)
            .and_then(|track| self.binding_for_track(track.id))
    }

    /// 读取当前节点的主导绑定。
    pub fn primary_binding<'a>(&'a self, graph: &'a MusicGraph) -> Option<&'a TrackBinding> {
        self.completion_source
            .and_then(|track_id| self.binding_for_track(track_id))
            .or_else(|| {
                graph
                    .main_track()
                    .and_then(|track| self.binding_for_track(track.id))
            })
            .or_else(|| self.bindings.first())
    }

    /// 读取当前节点的主导播放目标。
    pub fn primary_target<'a>(&'a self, graph: &'a MusicGraph) -> Option<&'a PlaybackTarget> {
        self.primary_binding(graph)
            .map(|binding| &binding.target)
            .or(Some(&self.target))
    }
}

/// 一条播放层定义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: SmolStr,
    #[serde(default)]
    pub role: TrackRole,
}

impl Track {
    /// 创建一个新的 track。
    pub fn new(name: impl Into<SmolStr>, role: TrackRole) -> Self {
        Self {
            id: TrackId::new(),
            name: name.into(),
            role,
        }
    }
}

/// 一条 track 的最小职责类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TrackRole {
    #[default]
    Main,
    Bridge,
    Stinger,
    Layer,
}

/// 把某个播放目标绑定到一条 track。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackBinding {
    pub track_id: TrackId,
    pub target: PlaybackTarget,
}

/// 一个节点最终绑定的播放目标。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlaybackTarget {
    Clip { clip_id: ClipId },
}

impl PlaybackTarget {
    /// 读取这个播放目标直接引用的 clip ID。
    pub fn clip_ids(&self) -> [ClipId; 1] {
        match self {
            Self::Clip { clip_id } => [*clip_id],
        }
    }
}

/// 进入目标内容时的默认策略。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EntryPolicy {
    #[default]
    ClipStart,
    Resume,
    ResumeNextMatchingCue {
        tag: SmolStr,
    },
    EntryCue {
        tag: SmolStr,
    },
    SameSyncPosition,
}

/// 一条边的触发方式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EdgeTrigger {
    #[default]
    Immediate,
    NextMatchingCue {
        tag: SmolStr,
    },
    OnComplete,
}

/// 记忆恢复策略。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryPolicy {
    pub ttl_seconds: Option<f32>,
    pub reset_to: EntryPolicy,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            ttl_seconds: None,
            reset_to: EntryPolicy::ClipStart,
        }
    }
}

/// 一条从源节点到目标节点的边。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicEdge {
    pub from: MusicNodeId,
    pub to: MusicNodeId,
    #[serde(default)]
    pub exit: Option<EdgeTrigger>,
    #[serde(default)]
    pub bridge_clip: Option<ClipId>,
    #[serde(default)]
    pub stinger_clip: Option<ClipId>,
    #[serde(default)]
    pub requested_target: Option<MusicNodeId>,
    #[serde(default)]
    pub trigger: EdgeTrigger,
    #[serde(default)]
    pub destination: EntryPolicy,
}

impl MusicEdge {
    /// 读取当前边真正使用的触发方式。
    pub fn effective_trigger(&self) -> &EdgeTrigger {
        self.exit.as_ref().unwrap_or(&self.trigger)
    }
}

pub type MusicStateNode = MusicNode;
pub type TransitionRule = MusicEdge;
pub type ExitPolicy = EdgeTrigger;
