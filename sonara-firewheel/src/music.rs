// SPDX-License-Identifier: MPL-2.0

use std::collections::{HashMap, HashSet};

use crate::{
    backend::FirewheelBackend,
    error::FirewheelBackendError,
    types::{
        MUSIC_SCHEDULE_EARLY_SECONDS, PendingExitCue, PendingMusicPlayback, PendingNodeCompletion,
        ResolvedClipPlayback, normalize_fade_duration_seconds, validate_playback_position_seconds,
    },
};
use firewheel::nodes::sampler::RepeatMode;
use sonara_model::{BusId, ClipId, MusicGraphId, MusicNodeId, TrackGroupId, TrackId};
use sonara_runtime::{
    Fade, MusicPhase, MusicStatus, ResolvedMusicPlayback, RuntimeError, TrackGroupState,
};

impl FirewheelBackend {
    /// 启动一个音乐图会话，使用图中声明的初始节点。
    pub fn play_music_graph(
        &mut self,
        graph_id: MusicGraphId,
    ) -> Result<sonara_runtime::MusicSessionId, FirewheelBackendError> {
        let session_id = self.runtime.play_music_graph(graph_id)?;
        self.sync_music_session_playback(session_id)?;
        Ok(session_id)
    }

    /// 启动一个音乐图会话，并显式指定初始节点。
    pub fn play_music_graph_in_node(
        &mut self,
        graph_id: MusicGraphId,
        initial_node: MusicNodeId,
    ) -> Result<sonara_runtime::MusicSessionId, FirewheelBackendError> {
        let session_id = self
            .runtime
            .play_music_graph_in_node(graph_id, Some(initial_node))?;
        self.sync_music_session_playback(session_id)?;
        Ok(session_id)
    }

    /// 请求一个音乐会话切换到目标节点。
    pub fn request_music_node(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
        target_node: MusicNodeId,
    ) -> Result<(), FirewheelBackendError> {
        self.save_music_session_resume_position(session_id)?;
        self.runtime.request_music_node(session_id, target_node)?;
        self.sync_music_session_playback(session_id)?;
        Ok(())
    }

    /// 通知后端：音乐会话已到达允许退出的切点。
    pub fn complete_music_exit(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
    ) -> Result<(), FirewheelBackendError> {
        self.save_music_session_resume_position(session_id)?;
        self.runtime.complete_music_exit(session_id)?;
        self.sync_music_session_playback(session_id)?;
        Ok(())
    }

    /// 通知后端：当前完成节点已经结束，可以进入目标节点。
    pub fn complete_music_node_completion(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
    ) -> Result<(), FirewheelBackendError> {
        self.runtime.complete_music_node_completion(session_id)?;
        self.sync_music_session_playback(session_id)?;
        Ok(())
    }

    /// 停止一个音乐会话。
    pub fn stop_music_session(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
        fade: Fade,
    ) -> Result<(), FirewheelBackendError> {
        self.save_music_session_resume_position(session_id)?;
        self.runtime.stop_music_session(session_id, fade)?;
        self.pending_music_playbacks.remove(&session_id);
        self.stop_music_session_workers(session_id, normalize_fade_duration_seconds(fade));
        self.active_music_clips.remove(&session_id);
        self.active_music_tracks.remove(&session_id);
        self.active_music_binding_clips.remove(&session_id);
        self.update()?;
        Ok(())
    }

    /// 查询音乐会话当前对游戏侧可见的状态。
    pub fn music_status(
        &self,
        session_id: sonara_runtime::MusicSessionId,
    ) -> Result<MusicStatus, FirewheelBackendError> {
        Ok(self.runtime.music_status(session_id)?)
    }

    /// 查询一个音乐会话中某个显式 track group 的当前状态。
    pub fn music_track_group_state(
        &self,
        session_id: sonara_runtime::MusicSessionId,
        group_id: TrackGroupId,
    ) -> Result<TrackGroupState, FirewheelBackendError> {
        Ok(self.runtime.music_track_group_state(session_id, group_id)?)
    }

    /// 设置一个音乐会话中某个显式 track group 的开关状态。
    pub fn set_music_track_group_active(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
        group_id: TrackGroupId,
        active: bool,
    ) -> Result<(), FirewheelBackendError> {
        let preserved_entry_offset_seconds =
            self.current_music_session_entry_offset_seconds(session_id)?;
        self.runtime
            .set_music_track_group_active(session_id, group_id, active)?;
        self.sync_music_session_playback_with_offset(session_id, preserved_entry_offset_seconds)?;
        Ok(())
    }

    /// 当前音乐会话是否还在等待媒体资源就绪。
    pub fn music_session_pending_media(&self, session_id: sonara_runtime::MusicSessionId) -> bool {
        self.pending_music_playbacks.contains_key(&session_id)
    }

    /// 读取音乐会话当前的代表性播放头秒数。
    pub fn music_session_playhead_seconds(
        &self,
        session_id: sonara_runtime::MusicSessionId,
    ) -> Option<f64> {
        self.music_session_playhead(session_id)
            .map(|playhead| playhead.position_seconds)
    }

    pub(crate) fn sync_music_session_playback(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
    ) -> Result<(), FirewheelBackendError> {
        self.sync_music_session_playback_with_offset(session_id, None)
    }

    pub(crate) fn sync_music_session_playback_with_offset(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
        entry_offset_override: Option<f64>,
    ) -> Result<(), FirewheelBackendError> {
        let status = self.runtime.music_status(session_id)?;

        match status.phase {
            MusicPhase::Stopped => {
                self.pending_music_playbacks.remove(&session_id);
                self.pending_exit_cues.remove(&session_id);
                self.pending_node_completions.remove(&session_id);
                self.stop_music_session_workers(session_id, 0.0);
                self.active_music_clips.remove(&session_id);
                self.active_music_tracks.remove(&session_id);
                self.active_music_binding_clips.remove(&session_id);
                self.update()?;
            }
            MusicPhase::WaitingExitCue => {
                self.pending_node_completions.remove(&session_id);
                if !self.ensure_waiting_exit_cue(session_id)? {
                    self.complete_music_exit(session_id)?;
                }
            }
            MusicPhase::Stable | MusicPhase::EnteringDestination => {
                self.pending_exit_cues.remove(&session_id);
                self.pending_node_completions.remove(&session_id);
                let mut resolved_music = match self
                    .runtime
                    .resolve_music_playback(session_id, self.audio_clock_seconds())
                {
                    Ok(resolved_music) => resolved_music,
                    Err(RuntimeError::MusicNodeHasNoActiveTrack { .. }) => {
                        self.pending_music_playbacks.remove(&session_id);
                        self.stop_music_session_workers(session_id, 0.0);
                        self.active_music_clips.remove(&session_id);
                        self.active_music_tracks.remove(&session_id);
                        self.active_music_binding_clips.remove(&session_id);
                        self.update()?;
                        return Ok(());
                    }
                    Err(err) => return Err(err.into()),
                };
                let mut resolved_playbacks = self
                    .runtime
                    .resolve_music_node_playbacks(session_id, self.audio_clock_seconds())?;
                if let Some(entry_offset_seconds) = entry_offset_override {
                    resolved_music.entry_offset_seconds = entry_offset_seconds;
                    for playback in &mut resolved_playbacks {
                        playback.entry_offset_seconds = entry_offset_seconds;
                    }
                }

                if self.active_music_clips.get(&session_id) == Some(&resolved_music.clip_id)
                    && self.active_music_tracks.get(&session_id).copied().flatten()
                        == resolved_music.track_id
                    && self
                        .music_session_matches_resolved_playbacks(session_id, &resolved_playbacks)
                    && self.music_session_has_live_worker(session_id)
                    && !self.pending_music_playbacks.contains_key(&session_id)
                {
                    return Ok(());
                }

                self.start_music_session_playbacks(session_id, resolved_music, resolved_playbacks)?;
            }
            MusicPhase::WaitingNodeCompletion => {
                self.pending_exit_cues.remove(&session_id);
                let mut resolved_music = match self
                    .runtime
                    .resolve_music_playback(session_id, self.audio_clock_seconds())
                {
                    Ok(resolved_music) => resolved_music,
                    Err(RuntimeError::MusicNodeHasNoActiveTrack { .. }) => {
                        self.pending_music_playbacks.remove(&session_id);
                        self.stop_music_session_workers(session_id, 0.0);
                        self.active_music_clips.remove(&session_id);
                        self.active_music_tracks.remove(&session_id);
                        self.active_music_binding_clips.remove(&session_id);
                        self.update()?;
                        return Ok(());
                    }
                    Err(err) => return Err(err.into()),
                };
                let mut resolved_playbacks = self
                    .runtime
                    .resolve_music_node_playbacks(session_id, self.audio_clock_seconds())?;
                if let Some(entry_offset_seconds) = entry_offset_override {
                    resolved_music.entry_offset_seconds = entry_offset_seconds;
                    for playback in &mut resolved_playbacks {
                        playback.entry_offset_seconds = entry_offset_seconds;
                    }
                }

                if self.active_music_clips.get(&session_id) == Some(&resolved_music.clip_id)
                    && self.active_music_tracks.get(&session_id).copied().flatten()
                        == resolved_music.track_id
                    && self
                        .music_session_matches_resolved_playbacks(session_id, &resolved_playbacks)
                    && self.music_session_has_live_worker(session_id)
                    && !self.pending_music_playbacks.contains_key(&session_id)
                {
                    return Ok(());
                }

                self.start_music_session_playbacks(session_id, resolved_music, resolved_playbacks)?;
            }
        }

        Ok(())
    }

    pub(crate) fn start_music_session_playbacks(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
        primary_playback: ResolvedMusicPlayback,
        playbacks: Vec<ResolvedMusicPlayback>,
    ) -> Result<(), FirewheelBackendError> {
        let Some(primary_track_id) = primary_playback.track_id else {
            return Ok(());
        };

        let mut resolved_tracks = Vec::with_capacity(playbacks.len());
        let mut all_ready = true;
        for playback in &playbacks {
            let resolved =
                self.resolve_clip_playback(playback.clip_id, playback.entry_offset_seconds)?;
            if !self.prepare_asset_for_playback(resolved.asset_id)? {
                all_ready = false;
            }
            resolved_tracks.push((playback.clone(), resolved));
        }

        if !all_ready {
            self.pending_music_playbacks.insert(
                session_id,
                PendingMusicPlayback {
                    primary_clip_id: primary_playback.clip_id,
                    primary_track_id: primary_playback.track_id,
                    playbacks: playbacks.clone(),
                },
            );
            return Ok(());
        }

        self.pending_music_playbacks.remove(&session_id);
        self.flush_finished_workers()?;
        let waiting_node_completion = matches!(
            self.runtime.music_status(session_id),
            Ok(status) if status.phase == MusicPhase::WaitingNodeCompletion
        );
        let start_audio_time_seconds = self.audio_clock_seconds();
        let target_binding_clips = self.resolved_music_playbacks_by_track(&playbacks);
        let current_binding_clips = self
            .active_music_binding_clips
            .get(&session_id)
            .cloned()
            .unwrap_or_default();
        let current_track_workers = self
            .music_session_track_workers
            .get(&session_id)
            .cloned()
            .unwrap_or_default();

        let mut tracks_to_keep = HashSet::new();
        for (&track_id, &clip_id) in &target_binding_clips {
            if current_binding_clips.get(&track_id) == Some(&clip_id)
                && self.music_session_track_has_live_worker(session_id, track_id)
            {
                tracks_to_keep.insert(track_id);
            }
        }

        let mut workers_to_stop = Vec::new();
        for (track_id, worker_ids) in &current_track_workers {
            if !tracks_to_keep.contains(track_id) {
                workers_to_stop.extend(worker_ids.iter().copied());
            }
        }

        let mut primary_resolved_clip = None;
        for (playback, resolved) in resolved_tracks {
            let Some(track_id) = playback.track_id else {
                continue;
            };
            if tracks_to_keep.contains(&track_id) {
                continue;
            }
            let schedule_internal_stop =
                !waiting_node_completion || Some(track_id) != Some(primary_track_id);
            self.play_music_clip_resolved(session_id, track_id, resolved, schedule_internal_stop)?;
            if Some(track_id) == Some(primary_track_id) {
                primary_resolved_clip = Some(resolved);
            }
        }

        if !workers_to_stop.is_empty() {
            self.stop_music_session_worker_ids(session_id, workers_to_stop, 0.0);
        }

        if let Some(primary_resolved_clip) = primary_resolved_clip {
            self.schedule_node_completion(
                session_id,
                primary_resolved_clip,
                start_audio_time_seconds,
            )?;
        } else if !waiting_node_completion {
            self.pending_node_completions.remove(&session_id);
        } else if !tracks_to_keep.contains(&primary_track_id) {
            self.pending_node_completions.remove(&session_id);
        }
        self.active_music_clips
            .insert(session_id, primary_playback.clip_id);
        self.active_music_tracks
            .insert(session_id, primary_playback.track_id);
        self.active_music_binding_clips
            .insert(session_id, target_binding_clips);
        self.update()?;
        Ok(())
    }

    pub(crate) fn play_music_clip_resolved(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
        track_id: TrackId,
        resolved: ResolvedClipPlayback,
        schedule_internal_stop: bool,
    ) -> Result<(), FirewheelBackendError> {
        let bus_id: Option<BusId> = self.runtime.music_track_output_bus(session_id, track_id)?;
        let bus_volume = bus_id
            .and_then(|bus_id| self.runtime.bus_gain(bus_id))
            .unwrap_or(1.0);
        let worker_id = self.play_clip_worker(
            resolved.asset_id,
            bus_id,
            bus_volume,
            resolved.start_from_seconds,
            resolved.repeat_mode,
            None,
        )?;
        self.attach_music_session_worker(session_id, track_id, worker_id);

        if schedule_internal_stop {
            if let Some(stop_after_seconds) = resolved.stop_after_seconds {
                let stop_time = Some(self.event_instant_after_seconds(stop_after_seconds));
                self.sampler_pool
                    .stop(worker_id, stop_time, &mut self.context);
            }
        }
        Ok(())
    }

    pub(crate) fn resolve_clip_playback(
        &self,
        clip_id: ClipId,
        entry_offset_seconds: f64,
    ) -> Result<ResolvedClipPlayback, FirewheelBackendError> {
        let clip = self
            .runtime
            .clip(clip_id)
            .ok_or(FirewheelBackendError::ClipNotLoaded(clip_id))?;
        let clip_base_seconds = clip
            .source_range
            .map(|range| range.start_seconds as f64)
            .unwrap_or(0.0);
        let clip_base_seconds = validate_playback_position_seconds(clip_base_seconds)?;
        let entry_offset_seconds = validate_playback_position_seconds(entry_offset_seconds)?;
        let start_from_seconds = clip_base_seconds + entry_offset_seconds;

        let stop_after_seconds = if let Some(range) = clip.source_range {
            let end_seconds = validate_playback_position_seconds(range.end_seconds as f64)?;
            if end_seconds < clip_base_seconds {
                return Err(FirewheelBackendError::InvalidClipRange(clip_id));
            }

            if end_seconds > start_from_seconds {
                Some(end_seconds - start_from_seconds)
            } else {
                Some(0.0)
            }
        } else {
            None
        };

        let repeat_mode = match clip.loop_range {
            Some(_) if clip.source_range.is_some() => {
                return Err(FirewheelBackendError::UnsupportedClipLoopRange(clip_id));
            }
            Some(_) => RepeatMode::RepeatEndlessly,
            None => RepeatMode::PlayOnce,
        };

        Ok(ResolvedClipPlayback {
            asset_id: clip.asset_id,
            start_from_seconds,
            stop_after_seconds,
            repeat_mode,
        })
    }

    pub(crate) fn ensure_waiting_exit_cue(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
    ) -> Result<bool, FirewheelBackendError> {
        if self.pending_exit_cues.contains_key(&session_id) {
            return Ok(true);
        }

        let Some(current_position_seconds) = self.music_session_local_playhead(session_id)? else {
            return Ok(true);
        };
        let Some(next_cue) = self
            .runtime
            .find_next_music_exit_cue(session_id, current_position_seconds)?
        else {
            return Ok(false);
        };
        let target_audio_time_seconds = self.target_audio_time_for_music_cue(
            session_id,
            current_position_seconds,
            next_cue.cue_position_seconds,
            next_cue.requires_wrap,
        )?;

        self.pending_exit_cues.insert(
            session_id,
            PendingExitCue {
                target_position_seconds: next_cue.cue_position_seconds,
                target_audio_time_seconds,
                waiting_for_wrap: next_cue.requires_wrap,
                last_position_seconds: current_position_seconds,
            },
        );
        Ok(true)
    }

    pub(crate) fn refresh_waiting_exit_cues(&mut self) -> Result<(), FirewheelBackendError> {
        let session_ids: HashSet<_> = self
            .pending_exit_cues
            .keys()
            .chain(self.pending_music_playbacks.keys())
            .chain(self.active_music_clips.keys())
            .copied()
            .collect();

        for session_id in session_ids {
            let waiting = matches!(
                self.runtime.music_status(session_id),
                Ok(status) if status.phase == MusicPhase::WaitingExitCue
            );
            if !waiting {
                self.pending_exit_cues.remove(&session_id);
                continue;
            }

            if self.pending_exit_cues.contains_key(&session_id) {
                continue;
            }

            if !self.ensure_waiting_exit_cue(session_id)? {
                self.complete_music_exit(session_id)?;
            }
        }

        Ok(())
    }

    pub(crate) fn schedule_node_completion(
        &mut self,
        session_id: sonara_runtime::MusicSessionId,
        resolved: ResolvedClipPlayback,
        start_audio_time_seconds: f64,
    ) -> Result<(), FirewheelBackendError> {
        let waiting_node_completion = matches!(
            self.runtime.music_status(session_id),
            Ok(status) if status.phase == MusicPhase::WaitingNodeCompletion
        );
        if !waiting_node_completion {
            self.pending_node_completions.remove(&session_id);
            return Ok(());
        }

        let Some(stop_after_seconds) = resolved.stop_after_seconds else {
            self.pending_node_completions.remove(&session_id);
            return Ok(());
        };

        self.pending_node_completions.insert(
            session_id,
            PendingNodeCompletion {
                target_audio_time_seconds: start_audio_time_seconds + stop_after_seconds,
            },
        );
        Ok(())
    }

    pub(crate) fn advance_pending_node_completions(&mut self) -> Result<(), FirewheelBackendError> {
        let session_ids: Vec<_> = self.pending_node_completions.keys().copied().collect();
        let mut ready_sessions = Vec::new();
        let now_seconds = self.audio_clock_seconds();

        for session_id in session_ids {
            let Some(pending) = self.pending_node_completions.get(&session_id).copied() else {
                continue;
            };

            let waiting_node_completion = matches!(
                self.runtime.music_status(session_id),
                Ok(status) if status.phase == MusicPhase::WaitingNodeCompletion
            );
            if !waiting_node_completion {
                self.pending_node_completions.remove(&session_id);
                continue;
            }

            if now_seconds + MUSIC_SCHEDULE_EARLY_SECONDS >= pending.target_audio_time_seconds {
                ready_sessions.push(session_id);
            }
        }

        for session_id in ready_sessions {
            self.pending_node_completions.remove(&session_id);
            self.complete_music_node_completion(session_id)?;
        }

        Ok(())
    }

    pub(crate) fn advance_waiting_exit_cues(&mut self) -> Result<(), FirewheelBackendError> {
        let session_ids: Vec<_> = self.pending_exit_cues.keys().copied().collect();
        let mut ready_sessions = Vec::new();
        let epsilon = 0.010;
        let now_seconds = self.audio_clock_seconds();

        for session_id in session_ids {
            let Some(mut pending) = self.pending_exit_cues.get(&session_id).copied() else {
                continue;
            };

            let waiting = matches!(
                self.runtime.music_status(session_id),
                Ok(status) if status.phase == MusicPhase::WaitingExitCue
            );
            if !waiting {
                self.pending_exit_cues.remove(&session_id);
                continue;
            }

            if let Some(target_audio_time_seconds) = pending.target_audio_time_seconds {
                if now_seconds + MUSIC_SCHEDULE_EARLY_SECONDS >= target_audio_time_seconds {
                    ready_sessions.push(session_id);
                    continue;
                }
            }

            let Some(current_position_seconds) = self.music_session_local_playhead(session_id)?
            else {
                continue;
            };

            if pending.waiting_for_wrap
                && current_position_seconds + epsilon < pending.last_position_seconds
            {
                pending.waiting_for_wrap = false;
            }

            if !pending.waiting_for_wrap
                && current_position_seconds + epsilon >= pending.target_position_seconds
            {
                ready_sessions.push(session_id);
            } else {
                pending.last_position_seconds = current_position_seconds;
                self.pending_exit_cues.insert(session_id, pending);
            }
        }

        for session_id in ready_sessions {
            self.pending_exit_cues.remove(&session_id);
            self.complete_music_exit(session_id)?;
        }

        Ok(())
    }

    pub(crate) fn resolved_music_playbacks_by_track(
        &self,
        playbacks: &[ResolvedMusicPlayback],
    ) -> HashMap<TrackId, ClipId> {
        playbacks
            .iter()
            .filter_map(|playback| {
                playback
                    .track_id
                    .map(|track_id| (track_id, playback.clip_id))
            })
            .collect()
    }

    pub(crate) fn music_session_matches_resolved_playbacks(
        &self,
        session_id: sonara_runtime::MusicSessionId,
        playbacks: &[ResolvedMusicPlayback],
    ) -> bool {
        self.active_music_binding_clips.get(&session_id)
            == Some(&self.resolved_music_playbacks_by_track(playbacks))
    }
}
