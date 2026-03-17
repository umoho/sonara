// SPDX-License-Identifier: MPL-2.0

use sonara_firewheel::FirewheelBackendError;
use sonara_runtime::RuntimeError;
use thiserror::Error;

/// Bevy 层统一暴露的 backend 错误。
#[derive(Debug, Error)]
pub enum AudioBackendError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Firewheel(#[from] FirewheelBackendError),
}
