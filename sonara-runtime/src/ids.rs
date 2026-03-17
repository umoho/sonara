// SPDX-License-Identifier: MPL-2.0

/// 运行时事件实例 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventInstanceId(pub(crate) u64);

/// 运行时 snapshot 实例 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotInstanceId(pub(crate) u64);

/// 运行时音乐会话 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MusicSessionId(pub(crate) u64);

/// 运行时 emitter ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EmitterId(pub(crate) u64);
