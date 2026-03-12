use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use crate::{
    BankId, BusId, ClipId, EventId, ImportSettings, MusicGraphId, ResumeSlotId, SnapshotId,
    StreamingMode, SyncDomainId,
};

/// authoring 层里的 bank 定义。
///
/// 它描述项目里“哪些高层对象应该被编进这个 bank”。
/// 运行时真正加载的仍然是下面的 `Bank`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankDefinition {
    pub id: BankId,
    pub name: SmolStr,
    #[serde(default)]
    pub events: Vec<EventId>,
    #[serde(default)]
    pub buses: Vec<BusId>,
    #[serde(default)]
    pub snapshots: Vec<SnapshotId>,
    #[serde(default)]
    pub music_graphs: Vec<MusicGraphId>,
}

impl BankDefinition {
    /// 创建一个新的 bank 定义。
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            id: BankId::new(),
            name: name.into(),
            events: Vec::new(),
            buses: Vec::new(),
            snapshots: Vec::new(),
            music_graphs: Vec::new(),
        }
    }
}

/// bank 中用于运行时加载资源的最小清单项
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankAsset {
    pub id: Uuid,
    pub name: SmolStr,
    pub source_path: Utf8PathBuf,
    pub import_settings: ImportSettings,
    pub streaming: StreamingMode,
}

/// compiled bank 中的媒体清单。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BankManifest {
    pub assets: Vec<BankAsset>,
    pub resident_media: Vec<Uuid>,
    pub streaming_media: Vec<Uuid>,
}

impl Default for BankManifest {
    fn default() -> Self {
        Self {
            assets: Vec::new(),
            resident_media: Vec::new(),
            streaming_media: Vec::new(),
        }
    }
}

/// compiled bank 中的高层对象清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankObjects {
    #[serde(default)]
    pub events: Vec<EventId>,
    #[serde(default)]
    pub buses: Vec<BusId>,
    #[serde(default)]
    pub snapshots: Vec<SnapshotId>,
    #[serde(default)]
    pub clips: Vec<ClipId>,
    #[serde(default)]
    pub resume_slots: Vec<ResumeSlotId>,
    #[serde(default)]
    pub sync_domains: Vec<SyncDomainId>,
    #[serde(default)]
    pub music_graphs: Vec<MusicGraphId>,
}

impl Default for BankObjects {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            buses: Vec::new(),
            snapshots: Vec::new(),
            clips: Vec::new(),
            resume_slots: Vec::new(),
            sync_domains: Vec::new(),
            music_graphs: Vec::new(),
        }
    }
}

/// 运行时的 bank 加载单元
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bank {
    pub id: BankId,
    pub name: SmolStr,
    pub objects: BankObjects,
    pub manifest: BankManifest,
}

impl Bank {
    /// 创建一个空 bank 定义
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            id: BankId::new(),
            name: name.into(),
            objects: BankObjects::default(),
            manifest: BankManifest::default(),
        }
    }
}
