use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use crate::{BankId, BusId, EventId, ImportSettings, SnapshotId, StreamingMode};

/// authoring 层里的 bank 定义。
///
/// 它描述项目里“哪些高层对象应该被编进这个 bank”。
/// 运行时真正加载的仍然是下面的 `Bank`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankDefinition {
    pub id: BankId,
    pub name: SmolStr,
    pub events: Vec<EventId>,
    pub buses: Vec<BusId>,
    pub snapshots: Vec<SnapshotId>,
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

/// 运行时的 bank 加载单元
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bank {
    pub id: BankId,
    pub name: SmolStr,
    pub events: Vec<EventId>,
    pub assets: Vec<BankAsset>,
    pub resident_media: Vec<Uuid>,
    pub streaming_media: Vec<Uuid>,
}

impl Bank {
    /// 创建一个空 bank 定义
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            id: BankId::new(),
            name: name.into(),
            events: Vec::new(),
            assets: Vec::new(),
            resident_media: Vec::new(),
            streaming_media: Vec::new(),
        }
    }
}
