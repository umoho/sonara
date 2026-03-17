// SPDX-License-Identifier: MPL-2.0

use firewheel::nodes::sampler::RepeatMode;
use firewheel_symphonium::DecodedAudio;
use sonara_model::{ClipId, TrackId};
use sonara_runtime::{
    AudioCommandOutcome, Fade, ResolvedMusicPlayback, RuntimeRequest, RuntimeRequestResult,
};
use uuid::Uuid;

use crate::error::FirewheelBackendError;

/// Firewheel backend 可消费的一条最小请求
pub type FirewheelRequest = RuntimeRequest;

/// Firewheel backend 执行请求后的结果
pub type FirewheelRequestResult = RuntimeRequestResult;

/// Firewheel backend 在隔离模式下处理单条请求后的结果
pub type FirewheelRequestOutcome =
    AudioCommandOutcome<FirewheelRequest, FirewheelRequestResult, FirewheelBackendError>;

pub(crate) const MUSIC_SCHEDULE_EARLY_SECONDS: f64 = 0.020;

/// 真实后端返回的一个实例播放头快照。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstancePlayhead {
    pub position_seconds: f64,
    pub worker_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingMusicPlayback {
    pub(crate) primary_clip_id: ClipId,
    pub(crate) primary_track_id: Option<TrackId>,
    pub(crate) playbacks: Vec<ResolvedMusicPlayback>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PendingExitCue {
    pub(crate) target_position_seconds: f64,
    pub(crate) target_audio_time_seconds: Option<f64>,
    pub(crate) waiting_for_wrap: bool,
    pub(crate) last_position_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PendingNodeCompletion {
    pub(crate) target_audio_time_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedClipPlayback {
    pub(crate) asset_id: Uuid,
    pub(crate) start_from_seconds: f64,
    pub(crate) stop_after_seconds: Option<f64>,
    pub(crate) repeat_mode: RepeatMode,
}

pub(crate) struct StreamingAssetLoadResult {
    pub(crate) asset_id: Uuid,
    pub(crate) result: Result<DecodedAudio, String>,
}

pub(crate) fn validate_playback_position_seconds(
    seconds: f64,
) -> Result<f64, FirewheelBackendError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(FirewheelBackendError::InvalidPlaybackPosition(seconds));
    }

    Ok(seconds)
}

pub(crate) fn validate_schedule_delay_seconds(seconds: f64) -> Result<f64, FirewheelBackendError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(FirewheelBackendError::InvalidScheduleDelay(seconds));
    }

    Ok(seconds)
}

pub(crate) fn normalize_fade_duration_seconds(fade: Fade) -> f64 {
    let duration_seconds = f64::from(fade.duration_seconds);
    if duration_seconds.is_finite() && duration_seconds > 0.0 {
        duration_seconds
    } else {
        0.0
    }
}
