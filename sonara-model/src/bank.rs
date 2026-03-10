use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use crate::{BankId, EventId};

/// 运行时的 bank 加载单元
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bank {
    pub id: BankId,
    pub name: SmolStr,
    pub events: Vec<EventId>,
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
            resident_media: Vec::new(),
            streaming_media: Vec::new(),
        }
    }
}
