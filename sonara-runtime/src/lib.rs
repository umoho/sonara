//! Sonara 的高层运行时接口

use sonara_model::{BankId, EventId, ParameterId, ParameterValue, SnapshotId};
use thiserror::Error;

/// 运行时事件实例 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventInstanceId(u64);

/// 运行时 snapshot 实例 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotInstanceId(u64);

/// 停止或切换时使用的淡变参数
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fade {
    pub duration_seconds: f32,
}

impl Fade {
    /// 立即切换, 不做淡变
    pub const IMMEDIATE: Self = Self {
        duration_seconds: 0.0,
    };

    /// 使用秒数构造淡变
    pub fn seconds(duration_seconds: f32) -> Self {
        Self { duration_seconds }
    }
}

/// 运行时错误
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime backend is not implemented yet")]
    NotImplemented,
    #[error("event `{0:?}` is not loaded")]
    EventNotLoaded(EventId),
    #[error("bank `{0:?}` is not loaded")]
    BankNotLoaded(BankId),
    #[error("parameter `{0:?}` is not available")]
    ParameterUnavailable(ParameterId),
}

/// 面向游戏逻辑的运行时入口
#[derive(Debug, Default)]
pub struct SonaraRuntime;

impl SonaraRuntime {
    /// 创建一个空运行时
    pub fn new() -> Self {
        Self
    }

    /// 加载一个 bank
    pub fn load_bank(&mut self, _bank_id: BankId) -> Result<BankId, RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }

    /// 播放一个未绑定实体的事件
    pub fn play(&self, _event_id: EventId) -> Result<EventInstanceId, RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }

    /// 停止一个事件实例
    pub fn stop(&mut self, _instance_id: EventInstanceId, _fade: Fade) -> Result<(), RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }

    /// 设置全局参数
    pub fn set_global_param(
        &mut self,
        _parameter_id: ParameterId,
        _value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }

    /// 压入一个 snapshot
    pub fn push_snapshot(
        &mut self,
        _snapshot_id: SnapshotId,
        _fade: Fade,
    ) -> Result<SnapshotInstanceId, RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }
}
