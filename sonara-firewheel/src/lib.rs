// SPDX-License-Identifier: MPL-2.0

//! Firewheel 后端适配层

mod assets;
mod backend;
mod error;
mod events;
mod music;
mod types;
mod workers;

pub use backend::FirewheelBackend;
pub use error::FirewheelBackendError;
pub use types::{
    FirewheelRequest, FirewheelRequestOutcome, FirewheelRequestResult, InstancePlayhead,
};
