// SPDX-License-Identifier: MPL-2.0

//! Bevy 集成层骨架

mod audio;
mod components;
mod error;
mod plugin;
pub mod prelude;
#[cfg(test)]
mod tests;

pub use audio::{AudioRequest, AudioRequestOutcome, AudioRequestResult, AudioUpdate, SonaraAudio};
pub use components::{AudioEmitter, AudioListener};
pub use error::AudioBackendError;
pub use plugin::{SonaraFirewheelPlugin, SonaraPlugin};
pub use sonara_runtime::{
    EventInstanceState, MusicPhase, MusicSessionId, MusicStatus, TrackGroupState,
};
