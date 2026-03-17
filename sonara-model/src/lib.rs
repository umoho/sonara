// SPDX-License-Identifier: MPL-2.0

//! Sonara 的核心数据模型
//!
//! 这一层只定义语义对象和序列化结构
//! 不依赖具体运行时后端, 引擎集成或编辑器状态

pub mod asset;
pub mod bank;
pub mod bus;
pub mod event;
pub mod ids;
pub mod music;
pub mod parameter;
pub mod project;
pub mod snapshot;
pub mod transport;

pub use asset::{AnalysisMetadata, AudioAsset, ImportSettings, LoopRegion, StreamingMode};
pub use bank::{Bank, BankAsset, BankDefinition, BankManifest, BankObjects};
pub use bus::Bus;
pub use event::{
    Event, EventContentNode, EventContentRoot, EventKind, LeafNode, LoopNode, NodeId, NodeRef,
    RandomNode, SamplerNode, SequenceNode, SpatialMode, SwitchCase, SwitchNode,
};
pub use ids::{
    BankId, BusId, ClipId, CueId, EventId, MusicGraphId, MusicNodeId, ParameterId, ResumeSlotId,
    SnapshotId, SyncDomainId, TrackGroupId, TrackId,
};
pub use music::{
    EdgeTrigger, EntryPolicy, MemoryPolicy, MusicEdge, MusicGraph, MusicNode, PlaybackTarget,
    Track, TrackBinding, TrackGroup, TrackGroupMode, TrackRole,
};
pub use parameter::{
    BoolParameter, EnumParameter, FloatParameter, Parameter, ParameterDefaultValue, ParameterKind,
    ParameterScope, ParameterValue,
};
pub use project::{AuthoringProject, ProjectFileError};
pub use snapshot::{Snapshot, SnapshotTarget};
pub use transport::{Clip, CuePoint, ResumeSlot, SyncDomain, SyncPoint, TimeRange};
