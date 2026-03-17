// SPDX-License-Identifier: MPL-2.0

use firewheel::{
    clock::EventInstant,
    nodes::sampler::{PlayFrom, RepeatMode, SamplerNode, SamplerState},
};
use firewheel_pool::WorkerID;
use sonara_model::{BusId, EventId};
use sonara_runtime::{EmitterId, EventInstanceId, Fade, PlaybackPlan};
use uuid::Uuid;

use crate::{
    backend::FirewheelBackend,
    error::FirewheelBackendError,
    types::{
        InstancePlayhead, normalize_fade_duration_seconds, validate_playback_position_seconds,
    },
};

impl FirewheelBackend {
    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, FirewheelBackendError> {
        let instance_id = self.runtime.play(event_id)?;
        let plan = self
            .runtime
            .active_plan(instance_id)
            .cloned()
            .expect("active plan should exist right after play");
        self.playback_plan(instance_id, &plan)?;
        Ok(instance_id)
    }

    /// 排队一个未绑定 emitter 的播放请求
    pub fn queue_play(&mut self, event_id: EventId) {
        self.command_buffer.queue_play(event_id);
    }

    /// 在 emitter 上播放一个事件
    pub fn play_on(
        &mut self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<EventInstanceId, FirewheelBackendError> {
        let instance_id = self.runtime.play_on(emitter_id, event_id)?;
        let plan = self
            .runtime
            .active_plan(instance_id)
            .cloned()
            .expect("active plan should exist right after play_on");
        self.playback_plan(instance_id, &plan)?;
        Ok(instance_id)
    }

    /// 排队一个面向 emitter 的播放请求
    pub fn queue_play_on(&mut self, emitter_id: EmitterId, event_id: EventId) {
        self.command_buffer.queue_play_on(emitter_id, event_id);
    }

    /// 停止一个事件实例。
    ///
    /// 当前最小实现只支持立即停止。非零 fade 先显式报错，避免制造已经支持淡出的假象。
    pub fn stop(
        &mut self,
        instance_id: EventInstanceId,
        fade: Fade,
    ) -> Result<(), FirewheelBackendError> {
        self.runtime.stop(instance_id, fade)?;
        self.pending_playbacks.remove(&instance_id);
        self.stop_event_instance_workers(instance_id, normalize_fade_duration_seconds(fade));
        self.update()?;
        Ok(())
    }

    /// 读取一个实例当前的代表性播放头。
    ///
    /// 如果这个实例绑定了多个 worker，则返回第一个 worker 的播放头，
    /// 并同时报告 worker 总数，供调用方决定是否需要更细粒度处理。
    pub fn instance_playhead(&self, instance_id: EventInstanceId) -> Option<InstancePlayhead> {
        let worker_ids = self.instance_workers.get(&instance_id)?;
        let worker_id = *worker_ids.first()?;
        let sample_rate = self.context.stream_info()?.sample_rate;
        let update_instant = self.context.audio_clock_instant();
        let state = self
            .sampler_pool
            .first_node_state::<SamplerState, _>(worker_id, &self.context)?;

        Some(InstancePlayhead {
            position_seconds: state
                .playhead_seconds_corrected(update_instant, sample_rate)
                .0,
            worker_count: worker_ids.len(),
        })
    }

    /// 把一个实例当前所有 worker 的播放头同步到指定秒数。
    pub fn seek_instance(
        &mut self,
        instance_id: EventInstanceId,
        position_seconds: f64,
    ) -> Result<bool, FirewheelBackendError> {
        let position_seconds = validate_playback_position_seconds(position_seconds)?;
        self.seek_instance_internal(instance_id, position_seconds, None)
    }

    /// 在未来音频时钟的某个时刻把实例播放头同步到指定秒数。
    pub fn seek_instance_after(
        &mut self,
        instance_id: EventInstanceId,
        position_seconds: f64,
        delay_seconds: f64,
    ) -> Result<bool, FirewheelBackendError> {
        let position_seconds = validate_playback_position_seconds(position_seconds)?;
        let delay_seconds = crate::types::validate_schedule_delay_seconds(delay_seconds)?;
        let start_time = Some(self.event_instant_after_seconds(delay_seconds));
        self.seek_instance_internal(instance_id, position_seconds, start_time)
    }

    fn seek_instance_internal(
        &mut self,
        instance_id: EventInstanceId,
        position_seconds: f64,
        start_time: Option<EventInstant>,
    ) -> Result<bool, FirewheelBackendError> {
        let worker_ids = self
            .instance_workers
            .get(&instance_id)
            .cloned()
            .unwrap_or_default();
        let mut changed = false;

        for worker_id in worker_ids {
            let Some(mut sampler) = self.sampler_pool.first_node(worker_id).cloned() else {
                continue;
            };

            sampler.start_from(PlayFrom::Seconds(position_seconds));
            changed |= self.sampler_pool.sync_worker_params(
                worker_id,
                &sampler,
                start_time,
                &mut self.context,
            );
        }

        if changed {
            self.update()?;
        }

        Ok(changed)
    }

    pub(crate) fn playback_plan(
        &mut self,
        instance_id: EventInstanceId,
        plan: &PlaybackPlan,
    ) -> Result<(), FirewheelBackendError> {
        if !self.is_playback_plan_ready(plan) {
            self.pending_playbacks.insert(instance_id, plan.clone());
            return Ok(());
        }

        self.pending_playbacks.remove(&instance_id);
        if let Some(worker_ids) = self.instance_workers.remove(&instance_id) {
            for worker_id in worker_ids {
                self.worker_instances.remove(&worker_id);
                self.worker_buses.remove(&worker_id);
            }
        }
        let bus_id = self.runtime.active_event_bus(instance_id);
        let bus_volume = self.runtime.active_bus_gain(instance_id).unwrap_or(1.0);

        for asset_id in &plan.asset_ids {
            self.play_asset(instance_id, *asset_id, bus_id, bus_volume, None, None)?;
        }

        self.update()?;
        Ok(())
    }

    pub(crate) fn play_clip_worker(
        &mut self,
        asset_id: Uuid,
        bus_id: Option<BusId>,
        bus_volume: f32,
        start_from_seconds: f64,
        repeat_mode: RepeatMode,
        start_time: Option<EventInstant>,
    ) -> Result<WorkerID, FirewheelBackendError> {
        self.prepare_asset_for_playback(asset_id)?;
        let resource = self
            .sample_resources
            .get(&asset_id)
            .cloned()
            .ok_or(FirewheelBackendError::AssetNotRegistered(asset_id))?;
        let mut sampler = SamplerNode::default();
        sampler.set_sample(resource);
        sampler.repeat_mode = repeat_mode;
        sampler.start_from(PlayFrom::Seconds(validate_playback_position_seconds(
            start_from_seconds,
        )?));

        let worker = self.sampler_pool.new_worker(
            &sampler,
            start_time,
            true,
            &mut self.context,
            |fx_chain, cx| {
                let mut params = fx_chain.fx_chain.volume_pan;
                params.set_volume_linear(bus_volume);
                fx_chain
                    .fx_chain
                    .set_params(params, None, &fx_chain.node_ids, cx);
                fx_chain.fx_chain.volume_pan = params;
            },
        )?;
        if let Some(old_worker_id) = worker.old_worker_id {
            self.finish_worker(old_worker_id);
        }

        self.bind_worker_to_bus(worker.worker_id, bus_id);

        Ok(worker.worker_id)
    }

    pub(crate) fn play_asset(
        &mut self,
        instance_id: EventInstanceId,
        asset_id: Uuid,
        bus_id: Option<BusId>,
        bus_volume: f32,
        start_from_seconds: Option<f64>,
        start_time: Option<EventInstant>,
    ) -> Result<(), FirewheelBackendError> {
        self.ensure_bank_asset_ready(asset_id)?;
        let resource = self
            .sample_resources
            .get(&asset_id)
            .cloned()
            .ok_or(FirewheelBackendError::AssetNotRegistered(asset_id))?;
        let mut sampler = SamplerNode::default();
        sampler.set_sample(resource);
        if let Some(start_from_seconds) = start_from_seconds {
            sampler.start_from(PlayFrom::Seconds(validate_playback_position_seconds(
                start_from_seconds,
            )?));
        } else {
            sampler.start_or_restart();
        }

        let worker = self.sampler_pool.new_worker(
            &sampler,
            start_time,
            true,
            &mut self.context,
            |fx_chain, cx| {
                let mut params = fx_chain.fx_chain.volume_pan;
                params.set_volume_linear(bus_volume);
                fx_chain
                    .fx_chain
                    .set_params(params, None, &fx_chain.node_ids, cx);
                fx_chain.fx_chain.volume_pan = params;
            },
        )?;
        self.attach_worker(instance_id, worker.worker_id);
        self.bind_worker_to_bus(worker.worker_id, bus_id);

        if let Some(old_worker_id) = worker.old_worker_id {
            self.finish_worker(old_worker_id);
        }

        Ok(())
    }
}
