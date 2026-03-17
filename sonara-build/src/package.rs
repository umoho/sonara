// SPDX-License-Identifier: MPL-2.0

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sonara_model::{Bank, Bus, Clip, Event, MusicGraph, ResumeSlot, Snapshot, SyncDomain};

use crate::error::CompiledBankFileError;

/// 一次 bank 编译后的最小载荷。
///
/// 它把 runtime/backend 加载一个 compiled bank 所需的高层对象定义放在一起，
/// 便于后续从文件读取后直接进入加载流程。
///
/// 当前 v0 阶段, 这个类型应被理解为:
///
/// - 当前 runtime 的最小加载载荷
/// - 当前 backend 的最小资源准备载荷
/// - 而不是最终固定不变的 bank 文件标准
///
/// 其中字段边界是:
///
/// - `bank.objects`
///   - 供 runtime 识别这个 bank 里有哪些高层对象
/// - `bank.manifest`
///   - 供 backend 准备媒体资源
/// - `events / buses / snapshots`
///   - 供 runtime 加载对象定义本体
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledBankPackage {
    pub bank: Bank,
    #[serde(default)]
    pub events: Vec<Event>,
    #[serde(default)]
    pub buses: Vec<Bus>,
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
    #[serde(default)]
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub resume_slots: Vec<ResumeSlot>,
    #[serde(default)]
    pub sync_domains: Vec<SyncDomain>,
    #[serde(default)]
    pub music_graphs: Vec<MusicGraph>,
}

impl CompiledBankPackage {
    /// 读取 runtime 当前真正会消费的 bank 元数据。
    pub fn bank(&self) -> &Bank {
        &self.bank
    }

    /// 读取 runtime 会加载的事件定义。
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// 读取 runtime 会加载的 bus 定义。
    pub fn buses(&self) -> &[Bus] {
        &self.buses
    }

    /// 读取 runtime 会加载的 snapshot 定义。
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }

    /// 读取 runtime 会加载的 clip 定义。
    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    /// 读取 runtime 会加载的记忆槽定义。
    pub fn resume_slots(&self) -> &[ResumeSlot] {
        &self.resume_slots
    }

    /// 读取 runtime 会加载的同步域定义。
    pub fn sync_domains(&self) -> &[SyncDomain] {
        &self.sync_domains
    }

    /// 读取 runtime 会加载的音乐图定义。
    pub fn music_graphs(&self) -> &[MusicGraph] {
        &self.music_graphs
    }

    /// 从 JSON 字符串读取 compiled bank 载荷。
    pub fn from_json_str(contents: &str) -> Result<Self, CompiledBankFileError> {
        Ok(serde_json::from_str(contents)?)
    }

    /// 把 compiled bank 载荷编码成格式化 JSON。
    pub fn to_json_string_pretty(&self) -> Result<String, CompiledBankFileError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// 从磁盘读取一个 JSON compiled bank 文件。
    pub fn read_json_file(path: impl AsRef<Path>) -> Result<Self, CompiledBankFileError> {
        let contents = fs::read_to_string(path)?;
        Self::from_json_str(&contents)
    }

    /// 把 compiled bank 载荷写到磁盘上的 JSON 文件。
    pub fn write_json_file(&self, path: impl AsRef<Path>) -> Result<(), CompiledBankFileError> {
        let contents = self.to_json_string_pretty()?;
        fs::write(path, contents)?;
        Ok(())
    }
}
