use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use crate::{ClipId, CueId, ResumeSlotId, SyncDomainId};

/// 一段基于秒的时间区间。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_seconds: f32,
    pub end_seconds: f32,
}

impl TimeRange {
    /// 使用秒数构造一个时间区间。
    pub fn new(start_seconds: f32, end_seconds: f32) -> Self {
        Self {
            start_seconds,
            end_seconds,
        }
    }
}

/// 用户 authoring 的切点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CuePoint {
    pub id: CueId,
    pub name: SmolStr,
    pub position_seconds: f32,
    #[serde(default)]
    pub tags: Vec<SmolStr>,
}

impl CuePoint {
    /// 创建一个新的 cue。
    pub fn new(name: impl Into<SmolStr>, position_seconds: f32) -> Self {
        Self {
            id: CueId::new(),
            name: name.into(),
            position_seconds,
            tags: Vec::new(),
        }
    }
}

/// 一个可播放的音乐片段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub name: SmolStr,
    pub asset_id: Uuid,
    pub source_range: Option<TimeRange>,
    pub loop_range: Option<TimeRange>,
    #[serde(default)]
    pub cues: Vec<CuePoint>,
    pub sync_domain: Option<SyncDomainId>,
}

impl Clip {
    /// 创建一个直接引用整段资源的新 clip。
    pub fn new(name: impl Into<SmolStr>, asset_id: Uuid) -> Self {
        Self {
            id: ClipId::new(),
            name: name.into(),
            asset_id,
            source_range: None,
            loop_range: None,
            cues: Vec::new(),
            sync_domain: None,
        }
    }
}

/// 一个可复用的播放头记忆槽。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeSlot {
    pub id: ResumeSlotId,
    pub name: SmolStr,
}

impl ResumeSlot {
    /// 创建一个新的记忆槽。
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            id: ResumeSlotId::new(),
            name: name.into(),
        }
    }
}

/// 同步结构域中的一个标记点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncPoint {
    pub name: SmolStr,
    pub position_seconds: f32,
    #[serde(default)]
    pub tags: Vec<SmolStr>,
}

impl SyncPoint {
    /// 创建一个新的同步标记点。
    pub fn new(name: impl Into<SmolStr>, position_seconds: f32) -> Self {
        Self {
            name: name.into(),
            position_seconds,
            tags: Vec::new(),
        }
    }
}

/// 一组结构可对齐的音乐内容。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncDomain {
    pub id: SyncDomainId,
    pub name: SmolStr,
    #[serde(default)]
    pub points: Vec<SyncPoint>,
}

impl SyncDomain {
    /// 创建一个新的同步域。
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            id: SyncDomainId::new(),
            name: name.into(),
            points: Vec::new(),
        }
    }
}
