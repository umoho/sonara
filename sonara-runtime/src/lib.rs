// SPDX-License-Identifier: MPL-2.0

//! Sonara 的高层运行时接口

mod bank;
mod commands;
mod error;
mod events;
mod ids;
mod mix;
mod music;
mod types;

pub use bank::SonaraRuntime;
pub use commands::{
    AudioCommandBuffer, AudioCommandOutcome, QueuedRuntime, RuntimeCommandBuffer, RuntimeRequest,
    RuntimeRequestResult,
};
pub use error::RuntimeError;
pub use ids::{EmitterId, EventInstanceId, MusicSessionId, SnapshotInstanceId};
pub use types::{
    ActiveEventInstance, ActiveMusicSession, ActiveSnapshotInstance, EventInstanceState, Fade,
    MusicPhase, MusicStatus, NextCueMatch, PendingMusicTransition, PlaybackPlan,
    ResolvedMusicPlayback, ResumeMemoryEntry, TrackGroupState,
};

#[cfg(test)]
mod tests;
