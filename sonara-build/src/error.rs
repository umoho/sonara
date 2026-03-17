// SPDX-License-Identifier: MPL-2.0

use sonara_model::ProjectFileError;
use thiserror::Error;

/// 构建阶段错误
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BuildError {
    #[error("事件内容树为空")]
    EmptyEventTree,
    #[error("事件根节点不存在")]
    MissingRootNode,
    #[error("事件内容树存在重复节点 ID")]
    DuplicateNodeId,
    #[error("节点引用了不存在的子节点")]
    MissingChildNode,
    #[error("容器节点必须至少包含一个子节点")]
    EmptyContainer,
    #[error("事件引用了不存在的音频资源")]
    MissingAudioAsset,
    #[error("bank 定义引用了不存在的事件")]
    MissingEventDefinition,
    #[error("bank 定义引用了不存在的 bus")]
    MissingBusDefinition,
    #[error("bank 定义引用了不存在的 snapshot")]
    MissingSnapshotDefinition,
    #[error("bank 定义引用了不存在的 music graph")]
    MissingMusicGraphDefinition,
    #[error("事件 switch 引用了不存在的参数")]
    MissingParameterDefinition,
    #[error("事件 switch 必须绑定枚举参数")]
    SwitchParameterNotEnum,
    #[error("事件 switch 引用了参数中不存在的枚举值")]
    UnknownSwitchVariant,
    #[error("music graph 必须至少包含一个 node")]
    EmptyMusicGraph,
    #[error("music graph 中存在重复 node ID")]
    DuplicateMusicNodeId,
    #[error("music graph 中存在重复 track ID")]
    DuplicateTrackId,
    #[error("music graph 中存在重复 track group ID")]
    DuplicateTrackGroupId,
    #[error("music graph 引用了不存在的 node")]
    MissingMusicNodeDefinition,
    #[error("music graph 引用了不存在的 track")]
    MissingTrackDefinition,
    #[error("music graph 引用了不存在的 track group")]
    MissingTrackGroupDefinition,
    #[error("music graph 引用了不存在的 clip")]
    MissingClipDefinition,
    #[error("music graph 引用了不存在的 resume slot")]
    MissingResumeSlotDefinition,
    #[error("clip 引用了不存在的 sync domain")]
    MissingSyncDomainDefinition,
    #[error("music node 中存在重复 track binding")]
    DuplicateTrackBinding,
    #[error("music node 必须至少绑定一个 playback target")]
    EmptyMusicNode,
    #[error("music node 的 completion_source 没有对应的 track binding")]
    MissingCompletionTrackBinding,
}

/// compiled bank 文件的最小 IO 错误。
#[derive(Debug, Error)]
pub enum CompiledBankFileError {
    #[error("读取 compiled bank 文件失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("compiled bank JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),
}

/// compiled bank 导出阶段错误。
#[derive(Debug, Error)]
pub enum ExportBankError {
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error(transparent)]
    File(#[from] CompiledBankFileError),
}

/// project 级 bank 构建阶段错误。
#[derive(Debug, Error)]
pub enum ProjectBuildError {
    #[error(transparent)]
    ProjectFile(#[from] ProjectFileError),
    #[error("project 中不存在名为 `{0}` 的 bank 定义")]
    MissingBankDefinition(String),
    #[error(transparent)]
    Build(#[from] BuildError),
}

/// project 级 bank 导出阶段错误。
#[derive(Debug, Error)]
pub enum ProjectExportBankError {
    #[error(transparent)]
    ProjectFile(#[from] ProjectFileError),
    #[error("project 中不存在名为 `{0}` 的 bank 定义")]
    MissingBankDefinition(String),
    #[error(transparent)]
    Export(#[from] ExportBankError),
}
