// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;

use firewheel::{
    clock::{DurationSeconds, EventInstant},
    nodes::sampler::SamplerState,
};
use firewheel_pool::WorkerID;
use sonara_model::{BusId, ClipId, TrackId};
use sonara_runtime::{EventInstanceId, Fade, MusicPhase, MusicSessionId};

use crate::{backend::FirewheelBackend, error::FirewheelBackendError, types::InstancePlayhead};

impl FirewheelBackend {
    pub(crate) fn music_session_playhead(
        &self,
        session_id: MusicSessionId,
    ) -> Option<InstancePlayhead> {
        let worker_ids = self.music_session_workers.get(&session_id)?;
        let sample_rate = self.context.stream_info()?.sample_rate;
        let update_instant = self.context.audio_clock_instant();
        let primary_worker_ids = self
            .active_music_tracks
            .get(&session_id)
            .and_then(|track_id| {
                track_id.and_then(|track_id| {
                    self.music_session_track_workers
                        .get(&session_id)?
                        .get(&track_id)
                })
            })
            .cloned();

        let candidate_worker_ids = primary_worker_ids.unwrap_or_else(|| worker_ids.clone());

        candidate_worker_ids.iter().find_map(|worker_id| {
            let state = self
                .sampler_pool
                .first_node_state::<SamplerState, _>(*worker_id, &self.context)?;

            Some(InstancePlayhead {
                position_seconds: state
                    .playhead_seconds_corrected(update_instant, sample_rate)
                    .0,
                worker_count: worker_ids.len(),
            })
        })
    }

    pub(crate) fn music_session_has_live_worker(&self, session_id: MusicSessionId) -> bool {
        let Some(expected_tracks) = self.active_music_binding_clips.get(&session_id) else {
            return false;
        };

        if expected_tracks.is_empty() {
            return false;
        }

        expected_tracks.keys().all(|track_id| {
            self.music_session_track_workers
                .get(&session_id)
                .and_then(|workers_by_track| workers_by_track.get(track_id))
                .is_some_and(|worker_ids| {
                    worker_ids
                        .iter()
                        .any(|worker_id| self.music_worker_is_live(*worker_id))
                })
        })
    }

    pub(crate) fn music_session_track_has_live_worker(
        &self,
        session_id: MusicSessionId,
        track_id: TrackId,
    ) -> bool {
        self.music_session_track_workers
            .get(&session_id)
            .and_then(|workers_by_track| workers_by_track.get(&track_id))
            .is_some_and(|worker_ids| {
                worker_ids
                    .iter()
                    .any(|worker_id| self.music_worker_is_live(*worker_id))
            })
    }

    pub(crate) fn music_worker_is_live(&self, worker_id: WorkerID) -> bool {
        self.sampler_pool
            .first_node_state::<SamplerState, _>(worker_id, &self.context)
            .is_some()
    }

    pub(crate) fn music_session_local_playhead(
        &self,
        session_id: MusicSessionId,
    ) -> Result<Option<f64>, FirewheelBackendError> {
        let Some(playhead) = self.music_session_playhead(session_id) else {
            return Ok(None);
        };
        let Some(clip_id) = self.active_music_clips.get(&session_id).copied() else {
            return Ok(None);
        };

        Ok(Some(self.clip_local_position_seconds(
            clip_id,
            playhead.position_seconds,
        )?))
    }

    pub(crate) fn bind_worker_to_bus(&mut self, worker_id: WorkerID, bus_id: Option<BusId>) {
        if let Some(bus_id) = bus_id {
            self.worker_buses.insert(worker_id, bus_id);
        } else {
            self.worker_buses.remove(&worker_id);
        }
    }

    pub(crate) fn sync_live_bus_gains(&mut self) -> bool {
        let bindings: Vec<_> = self
            .worker_buses
            .iter()
            .map(|(worker_id, bus_id)| (*worker_id, *bus_id))
            .collect();
        let mut changed = false;

        for (worker_id, bus_id) in bindings {
            let target_gain = self.runtime.bus_gain(bus_id).unwrap_or(1.0);
            changed |= self.set_worker_volume_linear(worker_id, target_gain, 0.0, None);
        }

        changed
    }

    pub(crate) fn attach_worker(&mut self, instance_id: EventInstanceId, worker_id: WorkerID) {
        self.worker_instances.insert(worker_id, instance_id);
        self.instance_workers
            .entry(instance_id)
            .or_default()
            .push(worker_id);
    }

    pub(crate) fn attach_music_session_worker(
        &mut self,
        session_id: MusicSessionId,
        track_id: TrackId,
        worker_id: WorkerID,
    ) {
        self.worker_music_sessions.insert(worker_id, session_id);
        self.worker_music_tracks.insert(worker_id, track_id);
        self.music_session_workers
            .entry(session_id)
            .or_default()
            .push(worker_id);
        self.music_session_track_workers
            .entry(session_id)
            .or_default()
            .entry(track_id)
            .or_default()
            .push(worker_id);
    }

    pub(crate) fn save_music_session_resume_position(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<bool, FirewheelBackendError> {
        let Some(playhead) = self.music_session_playhead(session_id) else {
            return Ok(false);
        };
        let Some(clip_id) = self.active_music_clips.get(&session_id).copied() else {
            return Ok(false);
        };
        let clip_local_seconds =
            self.clip_local_position_seconds(clip_id, playhead.position_seconds)?;
        Ok(self.runtime.save_music_session_resume_position(
            session_id,
            clip_local_seconds,
            self.audio_clock_seconds(),
        )?)
    }

    pub(crate) fn current_music_session_entry_offset_seconds(
        &self,
        session_id: MusicSessionId,
    ) -> Result<Option<f64>, FirewheelBackendError> {
        let Some(playhead) = self.music_session_playhead(session_id) else {
            return Ok(None);
        };
        let Some(clip_id) = self.active_music_clips.get(&session_id).copied() else {
            return Ok(None);
        };
        Ok(Some(self.clip_local_position_seconds(
            clip_id,
            playhead.position_seconds,
        )?))
    }

    pub(crate) fn clip_local_position_seconds(
        &self,
        clip_id: ClipId,
        asset_position_seconds: f64,
    ) -> Result<f64, FirewheelBackendError> {
        let clip = self
            .runtime
            .clip(clip_id)
            .ok_or(FirewheelBackendError::ClipNotLoaded(clip_id))?;
        let clip_base_seconds = clip
            .source_range
            .map(|range| range.start_seconds as f64)
            .unwrap_or(0.0);
        let clip_base_seconds =
            crate::types::validate_playback_position_seconds(clip_base_seconds)?;
        let asset_position_seconds =
            crate::types::validate_playback_position_seconds(asset_position_seconds)?;

        Ok((asset_position_seconds - clip_base_seconds).max(0.0))
    }

    pub(crate) fn clip_duration_seconds(
        &self,
        clip_id: ClipId,
    ) -> Result<Option<f64>, FirewheelBackendError> {
        let clip = self
            .runtime
            .clip(clip_id)
            .ok_or(FirewheelBackendError::ClipNotLoaded(clip_id))?;
        let clip_base_seconds = clip
            .source_range
            .map(|range| range.start_seconds as f64)
            .unwrap_or(0.0);
        let clip_base_seconds =
            crate::types::validate_playback_position_seconds(clip_base_seconds)?;

        if let Some(range) = clip.source_range {
            let end_seconds =
                crate::types::validate_playback_position_seconds(range.end_seconds as f64)?;
            return Ok(Some((end_seconds - clip_base_seconds).max(0.0)));
        }

        let Some(resource) = self.sample_resources.get(&clip.asset_id) else {
            return Ok(None);
        };
        let Some(sample_rate) = resource.sample_rate() else {
            return Ok(None);
        };
        let sample_rate = f64::from(sample_rate.get());
        if sample_rate <= 0.0 {
            return Ok(None);
        }

        Ok(Some(
            resource.len_frames() as f64 / sample_rate - clip_base_seconds,
        ))
    }

    pub(crate) fn target_audio_time_for_music_cue(
        &self,
        session_id: MusicSessionId,
        current_position_seconds: f64,
        cue_position_seconds: f64,
        requires_wrap: bool,
    ) -> Result<Option<f64>, FirewheelBackendError> {
        let Some(clip_id) = self.active_music_clips.get(&session_id).copied() else {
            return Ok(None);
        };
        let delta_seconds = if requires_wrap {
            let Some(clip_duration_seconds) = self.clip_duration_seconds(clip_id)? else {
                return Ok(None);
            };
            (clip_duration_seconds - current_position_seconds).max(0.0) + cue_position_seconds
        } else {
            (cue_position_seconds - current_position_seconds).max(0.0)
        };

        Ok(Some(self.audio_clock_seconds() + delta_seconds))
    }

    pub(crate) fn stop_event_instance_workers(
        &mut self,
        instance_id: EventInstanceId,
        fade_seconds: f64,
    ) {
        let worker_ids = self
            .instance_workers
            .remove(&instance_id)
            .unwrap_or_default();
        for worker_id in worker_ids {
            self.worker_instances.remove(&worker_id);
            self.worker_buses.remove(&worker_id);
            self.stop_worker(worker_id, fade_seconds);
        }
    }

    pub(crate) fn stop_music_session_workers(
        &mut self,
        session_id: MusicSessionId,
        fade_seconds: f64,
    ) {
        let worker_ids = self
            .music_session_workers
            .remove(&session_id)
            .unwrap_or_default();
        self.music_session_track_workers.remove(&session_id);
        for worker_id in worker_ids {
            self.worker_music_sessions.remove(&worker_id);
            self.worker_music_tracks.remove(&worker_id);
            self.worker_buses.remove(&worker_id);
            self.stop_worker(worker_id, fade_seconds);
        }
    }

    pub(crate) fn stop_music_session_worker_ids(
        &mut self,
        session_id: MusicSessionId,
        worker_ids: Vec<WorkerID>,
        fade_seconds: f64,
    ) {
        if worker_ids.is_empty() {
            return;
        }

        let worker_ids_to_stop: HashSet<_> = worker_ids.into_iter().collect();

        if let Some(session_worker_ids) = self.music_session_workers.get_mut(&session_id) {
            session_worker_ids.retain(|worker_id| !worker_ids_to_stop.contains(worker_id));
            if session_worker_ids.is_empty() {
                self.music_session_workers.remove(&session_id);
            }
        }

        for worker_id in worker_ids_to_stop {
            self.worker_music_sessions.remove(&worker_id);
            let removed_track_id = self.worker_music_tracks.remove(&worker_id);
            self.worker_buses.remove(&worker_id);

            if let Some(track_id) = removed_track_id {
                if let Some(workers_by_track) =
                    self.music_session_track_workers.get_mut(&session_id)
                {
                    if let Some(track_worker_ids) = workers_by_track.get_mut(&track_id) {
                        track_worker_ids.retain(|candidate| *candidate != worker_id);
                        if track_worker_ids.is_empty() {
                            workers_by_track.remove(&track_id);
                        }
                    }

                    if workers_by_track.is_empty() {
                        self.music_session_track_workers.remove(&session_id);
                    }
                }
            }

            self.stop_worker(worker_id, fade_seconds);
        }
    }

    pub(crate) fn stop_worker(&mut self, worker_id: WorkerID, fade_seconds: f64) {
        if fade_seconds > 0.0 {
            self.set_worker_volume_linear(worker_id, 0.0, fade_seconds, None);
            let stop_time = Some(self.event_instant_after_seconds(fade_seconds));
            self.sampler_pool
                .stop(worker_id, stop_time, &mut self.context);
        } else {
            self.sampler_pool.stop(worker_id, None, &mut self.context);
        }
    }

    pub(crate) fn set_worker_volume_linear(
        &mut self,
        worker_id: WorkerID,
        volume_linear: f32,
        smooth_seconds: f64,
        start_time: Option<EventInstant>,
    ) -> bool {
        let Some(fx_state) = self.sampler_pool.fx_chain_mut(worker_id) else {
            return false;
        };

        let target_volume_linear = volume_linear.max(0.0);
        let target_smooth_seconds = smooth_seconds.max(0.0) as f32;
        if (fx_state.fx_chain.volume_pan.volume.linear() - target_volume_linear).abs() <= 0.0001
            && (fx_state.fx_chain.volume_pan.smooth_seconds - target_smooth_seconds).abs() <= 0.0001
        {
            return false;
        }

        let mut params = fx_state.fx_chain.volume_pan;
        params.smooth_seconds = target_smooth_seconds;
        params.set_volume_linear(target_volume_linear);
        fx_state
            .fx_chain
            .set_params(params, start_time, &fx_state.node_ids, &mut self.context);
        fx_state.fx_chain.volume_pan = params;
        true
    }

    pub(crate) fn flush_finished_workers(&mut self) -> Result<(), FirewheelBackendError> {
        self.context
            .update()
            .map_err(|error| FirewheelBackendError::Update(format!("{error:?}")))?;
        let poll_result = self.sampler_pool.poll(&self.context);
        for worker_id in poll_result.finished_workers {
            self.finish_worker(worker_id);
        }
        Ok(())
    }

    pub(crate) fn finish_worker(&mut self, worker_id: WorkerID) {
        if self
            .sampler_pool
            .first_node_state::<SamplerState, _>(worker_id, &self.context)
            .is_some()
        {
            return;
        }

        if let Some(instance_id) = self.worker_instances.remove(&worker_id) {
            self.worker_buses.remove(&worker_id);
            if let Some(worker_ids) = self.instance_workers.get_mut(&instance_id) {
                worker_ids.retain(|id| *id != worker_id);

                if worker_ids.is_empty() {
                    self.instance_workers.remove(&instance_id);
                    let _ = self.runtime.stop(instance_id, Fade::IMMEDIATE);
                }
            }

            return;
        }

        let Some(session_id) = self.worker_music_sessions.remove(&worker_id) else {
            return;
        };
        self.worker_buses.remove(&worker_id);
        let mut bridge_finished = false;
        let removed_track_id = self.worker_music_tracks.remove(&worker_id);

        if let Some(worker_ids) = self.music_session_workers.get_mut(&session_id) {
            worker_ids.retain(|id| *id != worker_id);

            if worker_ids.is_empty() {
                self.music_session_workers.remove(&session_id);
                self.music_session_track_workers.remove(&session_id);
                self.active_music_clips.remove(&session_id);
                self.active_music_tracks.remove(&session_id);
                self.active_music_binding_clips.remove(&session_id);
                bridge_finished = matches!(
                    self.runtime.music_status(session_id),
                    Ok(status) if status.phase == MusicPhase::WaitingNodeCompletion
                );
            }
        }

        if let Some(track_id) = removed_track_id {
            if let Some(workers_by_track) = self.music_session_track_workers.get_mut(&session_id) {
                if let Some(worker_ids) = workers_by_track.get_mut(&track_id) {
                    worker_ids.retain(|id| *id != worker_id);
                    if worker_ids.is_empty() {
                        workers_by_track.remove(&track_id);
                    }
                }
                if workers_by_track.is_empty() {
                    self.music_session_track_workers.remove(&session_id);
                }
            }
        }

        if bridge_finished {
            let _ = self.complete_music_node_completion(session_id);
        }
    }

    pub(crate) fn event_instant_after_seconds(&self, delay_seconds: f64) -> EventInstant {
        EventInstant::Seconds(
            self.context.audio_clock_corrected().seconds + DurationSeconds(delay_seconds),
        )
    }
}
