// SPDX-License-Identifier: MPL-2.0

use firewheel_pool::NewWorkerError;
use sonara_build::BuildError;
use sonara_model::ClipId;
use sonara_runtime::RuntimeError;
use thiserror::Error;
use uuid::Uuid;

/// Firewheel 后端错误
#[derive(Debug, Error)]
pub enum FirewheelBackendError {
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("资源 `{0}` 没有注册到 Firewheel 后端")]
    AssetNotRegistered(Uuid),
    #[error("资源 `{0}` 的声道数必须大于 0")]
    InvalidChannelCount(Uuid),
    #[error("资源 `{0}` 的采样率必须大于 0")]
    InvalidSampleRate(Uuid),
    #[error("资源 `{0}` 解码失败: {1}")]
    DecodeAsset(Uuid, String),
    #[error("Firewheel 启动音频流失败: {0}")]
    StartStream(String),
    #[error("Firewheel 更新失败: {0}")]
    Update(String),
    #[error("Firewheel worker 创建失败: {0}")]
    NewWorker(#[from] NewWorkerError),
    #[error("播放位置 `{0}` 必须是非负有限秒数")]
    InvalidPlaybackPosition(f64),
    #[error("调度延迟 `{0}` 必须是非负有限秒数")]
    InvalidScheduleDelay(f64),
    #[error("clip `{0:?}` is not loaded")]
    ClipNotLoaded(ClipId),
    #[error("clip `{0:?}` 的时间范围非法")]
    InvalidClipRange(ClipId),
    #[error("clip `{0:?}` 的子区间循环暂未接入 Firewheel sampler")]
    UnsupportedClipLoopRange(ClipId),
}
