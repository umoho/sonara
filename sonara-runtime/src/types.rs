// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use sonara_model::{
    BusId, ClipId, EdgeTrigger, EntryPolicy, EventId, MusicGraphId, MusicNodeId, PlaybackTarget,
    SnapshotId, TrackGroupId, TrackId,
};
use uuid::Uuid;

use crate::ids::{EmitterId, EventInstanceId, MusicSessionId, SnapshotInstanceId};

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

/// 事件实例当前对游戏侧可见的播放状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventInstanceState {
    /// 实例已经建立, 但媒体还没准备到可实际发声。
    PendingMedia,
    /// 实例已经进入实际播放。
    Playing,
    /// 实例不存在或已经停止。
    Stopped,
}

/// 运行中的事件实例
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEventInstance {
    pub id: EventInstanceId,
    pub event_id: EventId,
    pub emitter_id: Option<EmitterId>,
    pub plan: PlaybackPlan,
}

/// 运行中的 snapshot 实例
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveSnapshotInstance {
    pub id: SnapshotInstanceId,
    pub snapshot_id: SnapshotId,
    pub fade: Fade,
    pub overrides: HashMap<BusId, f32>,
}

/// 音乐会话当前所处的逻辑阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicPhase {
    Stable,
    WaitingExitCue,
    WaitingNodeCompletion,
    EnteringDestination,
    Stopped,
}

/// 一条等待完成的音乐状态切换。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMusicTransition {
    pub from_node: MusicNodeId,
    pub to_node: MusicNodeId,
    pub requested_target_node: MusicNodeId,
    pub trigger: EdgeTrigger,
    pub destination: EntryPolicy,
}

/// 运行中的音乐会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMusicSession {
    pub id: MusicSessionId,
    pub graph_id: MusicGraphId,
    pub desired_target_node: MusicNodeId,
    pub active_node: MusicNodeId,
    pub current_entry: EntryPolicy,
    pub phase: MusicPhase,
    pub pending_transition: Option<PendingMusicTransition>,
    pub track_group_states: HashMap<TrackGroupId, TrackGroupState>,
}

/// 运行时某个 track group 的最小状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackGroupState {
    pub active: bool,
}

/// 对游戏逻辑暴露的音乐会话状态快照。
#[derive(Debug, Clone, PartialEq)]
pub struct MusicStatus {
    pub session_id: MusicSessionId,
    pub graph_id: MusicGraphId,
    pub desired_target_node: MusicNodeId,
    pub active_node: MusicNodeId,
    pub phase: MusicPhase,
    pub current_track_id: Option<TrackId>,
    pub current_target: Option<PlaybackTarget>,
    pub pending_transition: Option<PendingMusicTransition>,
    pub track_group_states: HashMap<TrackGroupId, TrackGroupState>,
}

/// 一个记忆槽当前保存的播放头。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResumeMemoryEntry {
    pub position_seconds: f64,
    pub saved_at_seconds: f64,
}

/// 运行时为当前音乐会话解析出的播放目标。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMusicPlayback {
    pub clip_id: ClipId,
    pub track_id: Option<TrackId>,
    pub entry_offset_seconds: f64,
}

/// 一次“从当前位置往后找最近匹配 cue”后的解析结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NextCueMatch {
    pub cue_position_seconds: f64,
    pub requires_wrap: bool,
}
