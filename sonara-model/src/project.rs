use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::{AudioAsset, BankDefinition, Bus, Event, Parameter, Snapshot};

/// authoring 层的项目根对象。
///
/// 这一层表达音频师在编辑器里维护的内容集合。
/// 运行时不直接加载它, 而是从中构建 compiled bank。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoringProject {
    pub name: SmolStr,
    pub assets: Vec<AudioAsset>,
    pub parameters: Vec<Parameter>,
    pub buses: Vec<Bus>,
    pub snapshots: Vec<Snapshot>,
    pub events: Vec<Event>,
    pub banks: Vec<BankDefinition>,
}

impl AuthoringProject {
    /// 创建一个空项目。
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            name: name.into(),
            assets: Vec::new(),
            parameters: Vec::new(),
            buses: Vec::new(),
            snapshots: Vec::new(),
            events: Vec::new(),
            banks: Vec::new(),
        }
    }
}
