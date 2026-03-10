use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::ids::BusId;

/// 混音层级中的 bus 定义
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bus {
    pub id: BusId,
    pub name: SmolStr,
    pub parent: Option<BusId>,
    pub default_volume: f32,
}

impl Bus {
    /// 创建一个默认音量为 1.0 的 bus
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            id: BusId::new(),
            name: name.into(),
            parent: None,
            default_volume: 1.0,
        }
    }
}
