use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::{ClipId, MusicGraphId, MusicStateId, ResumeSlotId, TrackId};

/// 一个音乐状态图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicGraph {
    pub id: MusicGraphId,
    pub name: SmolStr,
    pub initial_state: Option<MusicStateId>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub states: Vec<MusicStateNode>,
    #[serde(default)]
    pub transitions: Vec<TransitionRule>,
}

impl MusicGraph {
    /// 创建一个新的音乐图。
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            id: MusicGraphId::new(),
            name: name.into(),
            initial_state: None,
            tracks: Vec::new(),
            states: Vec::new(),
            transitions: Vec::new(),
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
}

/// 音乐图中的一个状态节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicStateNode {
    pub id: MusicStateId,
    pub name: SmolStr,
    pub target: PlaybackTarget,
    #[serde(default)]
    pub bindings: Vec<TrackBinding>,
    pub memory_slot: Option<ResumeSlotId>,
    #[serde(default)]
    pub memory_policy: MemoryPolicy,
    #[serde(default)]
    pub default_entry: EntryPolicy,
}

impl MusicStateNode {
    /// 读取当前状态绑定到指定 track 的目标。
    pub fn binding_for_track(&self, track_id: TrackId) -> Option<&TrackBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.track_id == track_id)
    }

    /// 读取当前状态在兼容模式下的主目标。
    ///
    /// 如果图中声明了主 track，且当前状态也为它绑定了内容，则优先返回该绑定；
    /// 否则回退到 legacy `target` 字段。
    pub fn primary_target<'a>(&'a self, graph: &'a MusicGraph) -> &'a PlaybackTarget {
        graph
            .main_track()
            .and_then(|track| {
                self.binding_for_track(track.id)
                    .map(|binding| &binding.target)
            })
            .unwrap_or(&self.target)
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

/// 一个状态最终绑定的播放目标。
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

/// 从源状态退出时的策略。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExitPolicy {
    #[default]
    Immediate,
    NextMatchingCue {
        tag: SmolStr,
    },
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

/// 一条从源状态到目标状态的切换规则。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionRule {
    pub from: MusicStateId,
    pub to: MusicStateId,
    #[serde(default)]
    pub exit: ExitPolicy,
    pub bridge_clip: Option<ClipId>,
    #[serde(default)]
    pub stinger_clip: Option<ClipId>,
    #[serde(default)]
    pub destination: EntryPolicy,
}
