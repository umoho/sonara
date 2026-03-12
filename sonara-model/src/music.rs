use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::{ClipId, MusicGraphId, MusicStateId, ResumeSlotId};

/// 一个音乐状态图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicGraph {
    pub id: MusicGraphId,
    pub name: SmolStr,
    pub initial_state: Option<MusicStateId>,
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
            states: Vec::new(),
            transitions: Vec::new(),
        }
    }
}

/// 音乐图中的一个状态节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicStateNode {
    pub id: MusicStateId,
    pub name: SmolStr,
    pub target: PlaybackTarget,
    pub memory_slot: Option<ResumeSlotId>,
    #[serde(default)]
    pub memory_policy: MemoryPolicy,
    #[serde(default)]
    pub default_entry: EntryPolicy,
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
    pub destination: EntryPolicy,
}
