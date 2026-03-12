use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

use crate::{
    AudioAsset, BankDefinition, Bus, Clip, Event, MusicGraph, Parameter, ResumeSlot, Snapshot,
    SyncDomain,
};

/// authoring 项目文件的最小 IO 错误。
#[derive(Debug, Error)]
pub enum ProjectFileError {
    #[error("读取项目文件失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("项目文件 JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),
}

/// authoring 层的项目根对象。
///
/// 这一层表达音频师在编辑器里维护的内容集合。
/// 运行时不直接加载它, 而是从中构建 compiled bank。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoringProject {
    pub name: SmolStr,
    #[serde(default)]
    pub assets: Vec<AudioAsset>,
    #[serde(default)]
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub resume_slots: Vec<ResumeSlot>,
    #[serde(default)]
    pub sync_domains: Vec<SyncDomain>,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    #[serde(default)]
    pub buses: Vec<Bus>,
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
    #[serde(default)]
    pub events: Vec<Event>,
    #[serde(default)]
    pub music_graphs: Vec<MusicGraph>,
    #[serde(default)]
    pub banks: Vec<BankDefinition>,
}

impl AuthoringProject {
    /// 创建一个空项目。
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            name: name.into(),
            assets: Vec::new(),
            clips: Vec::new(),
            resume_slots: Vec::new(),
            sync_domains: Vec::new(),
            parameters: Vec::new(),
            buses: Vec::new(),
            snapshots: Vec::new(),
            events: Vec::new(),
            music_graphs: Vec::new(),
            banks: Vec::new(),
        }
    }

    /// 从 JSON 字符串读取 authoring 项目。
    pub fn from_json_str(contents: &str) -> Result<Self, ProjectFileError> {
        Ok(serde_json::from_str(contents)?)
    }

    /// 把项目编码成格式化 JSON 字符串。
    pub fn to_json_string_pretty(&self) -> Result<String, ProjectFileError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// 从磁盘读取一个 JSON 项目文件。
    pub fn read_json_file(path: impl AsRef<Path>) -> Result<Self, ProjectFileError> {
        let contents = fs::read_to_string(path)?;
        Self::from_json_str(&contents)
    }

    /// 把项目写入磁盘上的 JSON 文件。
    pub fn write_json_file(&self, path: impl AsRef<Path>) -> Result<(), ProjectFileError> {
        let path = path.as_ref();
        let contents = self.to_json_string_pretty()?;

        fs::write(path, contents)?;
        Ok(())
    }

    /// 按名称查找一个 bank 定义。
    pub fn bank_named(&self, name: &str) -> Option<&BankDefinition> {
        self.banks.iter().find(|bank| bank.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::AuthoringProject;

    #[test]
    fn json_round_trip_preserves_project_name_and_bank_lookup() {
        let mut project = AuthoringProject::new("demo");
        project.banks.push(crate::BankDefinition::new("core"));

        let json = project
            .to_json_string_pretty()
            .expect("project should serialize");
        let decoded =
            AuthoringProject::from_json_str(&json).expect("project should deserialize from JSON");

        assert_eq!(decoded.name, "demo");
        assert!(decoded.bank_named("core").is_some());
    }
}
