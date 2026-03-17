// SPDX-License-Identifier: MPL-2.0

use bevy_ecs::prelude::NonSendMut;
use sonara_build::CompiledBankPackage;
use sonara_firewheel::FirewheelBackend;
use sonara_model::{
    Bank, BankId, Bus, BusEffectSlot, BusId, Clip, Event, EventId, MusicGraph, MusicGraphId,
    MusicNodeId, ParameterId, ParameterValue, ResumeSlot, Snapshot, SnapshotId, SyncDomain,
    TrackGroupId,
};
use sonara_runtime::{
    AudioCommandOutcome, EmitterId, EventInstanceId, EventInstanceState, Fade, MusicSessionId,
    MusicStatus, PlaybackPlan, QueuedRuntime, RuntimeRequest, RuntimeRequestResult,
    SnapshotInstanceId, SonaraRuntime, TrackGroupState,
};

use crate::{components::AudioEmitter, error::AudioBackendError};

/// Bevy 侧积累的一条音频请求
pub type AudioRequest = RuntimeRequest;

/// 一次请求执行后的结果
pub type AudioRequestResult = RuntimeRequestResult;

/// 一条请求在隔离执行模式下的结果
pub type AudioRequestOutcome =
    AudioCommandOutcome<AudioRequest, AudioRequestResult, AudioBackendError>;

/// 一次 update system 内部使用的最小音频命令上下文。
///
/// 调用方可以在一帧里不断向它排队请求，最后统一 `apply()`。
pub struct AudioUpdate<'a> {
    audio: &'a mut SonaraAudio,
}

enum SonaraBackend {
    Runtime(QueuedRuntime),
    Firewheel(FirewheelBackend),
}

/// Bevy 侧的全局音频入口
pub struct SonaraAudio {
    backend: SonaraBackend,
}

impl Default for SonaraAudio {
    fn default() -> Self {
        Self::new()
    }
}

impl SonaraAudio {
    /// 创建一个新的纯 runtime 音频入口。
    pub fn new() -> Self {
        Self {
            backend: SonaraBackend::Runtime(QueuedRuntime::new()),
        }
    }

    /// 创建一个接了 Firewheel 的音频入口。
    pub fn new_firewheel() -> Result<Self, AudioBackendError> {
        Ok(Self {
            backend: SonaraBackend::Firewheel(FirewheelBackend::new(Default::default())?),
        })
    }

    /// 当前是否运行在 Firewheel 模式。
    pub fn is_firewheel_enabled(&self) -> bool {
        matches!(self.backend, SonaraBackend::Firewheel(_))
    }

    /// 加载一个 bank。
    pub fn load_bank(
        &mut self,
        bank: Bank,
        events: Vec<Event>,
    ) -> Result<BankId, AudioBackendError> {
        self.load_bank_with_definitions(
            bank,
            events,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    /// 加载一个 bank 以及和它配套的高层对象定义。
    pub fn load_bank_with_definitions(
        &mut self,
        bank: Bank,
        events: Vec<Event>,
        buses: Vec<Bus>,
        snapshots: Vec<Snapshot>,
        clips: Vec<Clip>,
        resume_slots: Vec<ResumeSlot>,
        sync_domains: Vec<SyncDomain>,
        music_graphs: Vec<MusicGraph>,
    ) -> Result<BankId, AudioBackendError> {
        let bank_id = bank.id;

        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => {
                runtime.load_bank_with_definitions(
                    bank,
                    events,
                    buses,
                    snapshots,
                    clips,
                    resume_slots,
                    sync_domains,
                    music_graphs,
                )?;
            }
            SonaraBackend::Firewheel(backend) => {
                backend.load_bank_with_definitions(
                    bank,
                    events,
                    buses,
                    snapshots,
                    clips,
                    resume_slots,
                    sync_domains,
                    music_graphs,
                )?;
            }
        }

        Ok(bank_id)
    }

    /// 直接加载一份完整的 compiled bank 载荷。
    pub fn load_compiled_bank(
        &mut self,
        package: CompiledBankPackage,
    ) -> Result<BankId, AudioBackendError> {
        self.load_bank_with_definitions(
            package.bank,
            package.events,
            package.buses,
            package.snapshots,
            package.clips,
            package.resume_slots,
            package.sync_domains,
            package.music_graphs,
        )
    }

    /// 开始一次 update system 风格的音频更新。
    pub fn begin_update(&mut self) -> AudioUpdate<'_> {
        AudioUpdate { audio: self }
    }

    /// 在 Firewheel 模式下推进真实音频后端。
    pub fn update_backend(&mut self) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(_) => Ok(()),
            SonaraBackend::Firewheel(backend) => {
                backend.update()?;
                Ok(())
            }
        }
    }

    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => Ok(runtime.play(event_id)?),
            SonaraBackend::Firewheel(backend) => Ok(backend.play(event_id)?),
        }
    }

    /// 排队一个未绑定 emitter 的播放请求
    pub fn queue_play(&mut self, event_id: EventId) {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.queue_play(event_id),
            SonaraBackend::Firewheel(backend) => backend.queue_play(event_id),
        }
    }

    /// 创建一个 emitter
    pub fn create_emitter(&mut self) -> EmitterId {
        self.runtime_mut().create_emitter()
    }

    /// 确保一个 AudioEmitter 已绑定到底层 runtime emitter
    pub fn ensure_emitter(&mut self, emitter: &mut AudioEmitter) -> EmitterId {
        if let Some(id) = emitter.id {
            id
        } else {
            let id = self.runtime_mut().create_emitter();
            emitter.id = Some(id);
            id
        }
    }

    /// 释放一个 AudioEmitter 已绑定的 runtime emitter
    pub fn detach_emitter(&mut self, emitter: &mut AudioEmitter) -> Result<(), AudioBackendError> {
        if let Some(id) = emitter.id.take() {
            self.runtime_mut().remove_emitter(id)?;
        }

        Ok(())
    }

    /// 在 emitter 上播放一个事件
    pub fn play_on(
        &mut self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<EventInstanceId, AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => Ok(runtime.play_on(emitter_id, event_id)?),
            SonaraBackend::Firewheel(backend) => Ok(backend.play_on(emitter_id, event_id)?),
        }
    }

    /// 排队一个面向指定 emitter 的播放请求
    pub fn queue_play_on(&mut self, emitter_id: EmitterId, event_id: EventId) {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.queue_play_on(emitter_id, event_id),
            SonaraBackend::Firewheel(backend) => backend.queue_play_on(emitter_id, event_id),
        }
    }

    /// 通过 AudioEmitter 组件播放事件
    pub fn play_from_emitter(
        &mut self,
        emitter: &mut AudioEmitter,
        event_id: EventId,
    ) -> Result<EventInstanceId, AudioBackendError> {
        let emitter_id = self.ensure_emitter(emitter);
        self.play_on(emitter_id, event_id)
    }

    /// 通过 AudioEmitter 组件排队播放请求
    pub fn queue_play_from_emitter(&mut self, emitter: &mut AudioEmitter, event_id: EventId) {
        let emitter_id = self.ensure_emitter(emitter);
        self.queue_play_on(emitter_id, event_id);
    }

    /// 设置一个全局参数
    pub fn set_global_param(
        &mut self,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.set_global_param(parameter_id, value)?,
            SonaraBackend::Firewheel(backend) => {
                backend.set_global_param(parameter_id, value)?;
            }
        }

        Ok(())
    }

    /// 设置某个 bus 的 live gain。
    pub fn set_bus_gain(&mut self, bus_id: BusId, gain: f32) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.set_bus_gain(bus_id, gain)?,
            SonaraBackend::Firewheel(backend) => backend.set_bus_gain(bus_id, gain)?,
        }

        Ok(())
    }

    /// 替换某个 bus 上的一个 effect slot。
    pub fn set_bus_effect_slot(
        &mut self,
        bus_id: BusId,
        slot: BusEffectSlot,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.set_bus_effect_slot(bus_id, slot)?,
            SonaraBackend::Firewheel(backend) => backend.set_bus_effect_slot(bus_id, slot)?,
        }

        Ok(())
    }

    /// 排队一个全局参数更新请求
    pub fn queue_set_global_param(&mut self, parameter_id: ParameterId, value: ParameterValue) {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.queue_set_global_param(parameter_id, value),
            SonaraBackend::Firewheel(backend) => {
                backend.queue_set_global_param(parameter_id, value)
            }
        }
    }

    /// 设置 emitter 参数
    pub fn set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => {
                runtime.set_emitter_param(emitter_id, parameter_id, value)?;
            }
            SonaraBackend::Firewheel(backend) => {
                backend.set_emitter_param(emitter_id, parameter_id, value)?;
            }
        }

        Ok(())
    }

    /// 排队一个 emitter 参数更新请求
    pub fn queue_set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => {
                runtime.queue_set_emitter_param(emitter_id, parameter_id, value);
            }
            SonaraBackend::Firewheel(backend) => {
                backend.queue_set_emitter_param(emitter_id, parameter_id, value);
            }
        }
    }

    /// 通过 AudioEmitter 组件设置 emitter 参数
    pub fn set_emitter_param_on(
        &mut self,
        emitter: &mut AudioEmitter,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), AudioBackendError> {
        let emitter_id = self.ensure_emitter(emitter);
        self.set_emitter_param(emitter_id, parameter_id, value)
    }

    /// 通过 AudioEmitter 组件排队 emitter 参数更新
    pub fn queue_set_emitter_param_on(
        &mut self,
        emitter: &mut AudioEmitter,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) {
        let emitter_id = self.ensure_emitter(emitter);
        self.queue_set_emitter_param(emitter_id, parameter_id, value);
    }

    /// 排队一个停止实例请求
    pub fn queue_stop(&mut self, instance_id: EventInstanceId, fade: Fade) {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.queue_stop(instance_id, fade),
            SonaraBackend::Firewheel(backend) => backend.queue_stop(instance_id, fade),
        }
    }

    /// 读取一个事件实例当前解析出的播放计划
    pub fn active_plan(&self, instance_id: EventInstanceId) -> Option<&PlaybackPlan> {
        self.runtime().active_plan(instance_id)
    }

    /// 查询一个事件实例当前对游戏侧可见的播放状态。
    pub fn instance_state(&self, instance_id: EventInstanceId) -> EventInstanceState {
        match &self.backend {
            SonaraBackend::Runtime(runtime) => runtime.instance_state(instance_id),
            SonaraBackend::Firewheel(backend) => backend.instance_state(instance_id),
        }
    }

    /// 读取一个事件实例当前的代表性播放头。
    ///
    /// 纯 runtime 模式没有真实音频后端，因此返回 `None`。
    pub fn instance_playhead_seconds(&self, instance_id: EventInstanceId) -> Option<f64> {
        match &self.backend {
            SonaraBackend::Runtime(_) => None,
            SonaraBackend::Firewheel(backend) => backend
                .instance_playhead(instance_id)
                .map(|playhead| playhead.position_seconds),
        }
    }

    /// 启动一个音乐图会话，使用图中声明的初始节点。
    pub fn play_music_graph(
        &mut self,
        graph_id: MusicGraphId,
    ) -> Result<MusicSessionId, AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => Ok(runtime.play_music_graph(graph_id)?),
            SonaraBackend::Firewheel(backend) => Ok(backend.play_music_graph(graph_id)?),
        }
    }

    /// 启动一个音乐图会话，并显式指定初始节点。
    pub fn play_music_graph_in_node(
        &mut self,
        graph_id: MusicGraphId,
        initial_node: MusicNodeId,
    ) -> Result<MusicSessionId, AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => {
                Ok(runtime.play_music_graph_in_node(graph_id, Some(initial_node))?)
            }
            SonaraBackend::Firewheel(backend) => {
                Ok(backend.play_music_graph_in_node(graph_id, initial_node)?)
            }
        }
    }

    /// 请求一个音乐会话切换到目标节点。
    pub fn request_music_node(
        &mut self,
        session_id: MusicSessionId,
        target_node: MusicNodeId,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => {
                runtime.request_music_node(session_id, target_node)?;
            }
            SonaraBackend::Firewheel(backend) => {
                backend.request_music_node(session_id, target_node)?;
            }
        }

        Ok(())
    }

    /// 通知音乐会话：当前已经到达允许退出的切点。
    pub fn complete_music_exit(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.complete_music_exit(session_id)?,
            SonaraBackend::Firewheel(backend) => backend.complete_music_exit(session_id)?,
        }

        Ok(())
    }

    /// 通知音乐会话：当前自动推进节点已经播放完成。
    pub fn complete_music_node_completion(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => {
                runtime.complete_music_node_completion(session_id)?
            }
            SonaraBackend::Firewheel(backend) => {
                backend.complete_music_node_completion(session_id)?
            }
        }

        Ok(())
    }

    /// 停止一个音乐会话。
    pub fn stop_music_session(
        &mut self,
        session_id: MusicSessionId,
        fade: Fade,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.stop_music_session(session_id, fade)?,
            SonaraBackend::Firewheel(backend) => backend.stop_music_session(session_id, fade)?,
        }

        Ok(())
    }

    /// 查询音乐会话当前对游戏侧可见的状态。
    pub fn music_status(
        &self,
        session_id: MusicSessionId,
    ) -> Result<MusicStatus, AudioBackendError> {
        match &self.backend {
            SonaraBackend::Runtime(runtime) => Ok(runtime.music_status(session_id)?),
            SonaraBackend::Firewheel(backend) => Ok(backend.music_status(session_id)?),
        }
    }

    /// 查询一个音乐会话中某个显式 track group 的当前状态。
    pub fn music_track_group_state(
        &self,
        session_id: MusicSessionId,
        group_id: TrackGroupId,
    ) -> Result<TrackGroupState, AudioBackendError> {
        match &self.backend {
            SonaraBackend::Runtime(runtime) => {
                Ok(runtime.music_track_group_state(session_id, group_id)?)
            }
            SonaraBackend::Firewheel(backend) => {
                Ok(backend.music_track_group_state(session_id, group_id)?)
            }
        }
    }

    /// 设置一个音乐会话中某个显式 track group 的开关状态。
    pub fn set_music_track_group_active(
        &mut self,
        session_id: MusicSessionId,
        group_id: TrackGroupId,
        active: bool,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => {
                runtime.set_music_track_group_active(session_id, group_id, active)?
            }
            SonaraBackend::Firewheel(backend) => {
                backend.set_music_track_group_active(session_id, group_id, active)?
            }
        }

        Ok(())
    }

    /// 当前音乐会话是否还在等待媒体资源就绪。
    pub fn music_session_pending_media(&self, session_id: MusicSessionId) -> bool {
        match &self.backend {
            SonaraBackend::Runtime(_) => false,
            SonaraBackend::Firewheel(backend) => backend.music_session_pending_media(session_id),
        }
    }

    /// 读取音乐会话当前的代表性播放头秒数。
    pub fn music_session_playhead_seconds(&self, session_id: MusicSessionId) -> Option<f64> {
        match &self.backend {
            SonaraBackend::Runtime(_) => None,
            SonaraBackend::Firewheel(backend) => backend.music_session_playhead_seconds(session_id),
        }
    }

    /// 取出当前所有待处理请求
    pub fn drain_requests(&mut self) -> Vec<AudioRequest> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.drain_requests(),
            SonaraBackend::Firewheel(backend) => backend.drain_requests(),
        }
    }

    /// 依次执行所有待处理请求
    pub fn apply_requests(&mut self) -> Result<Vec<AudioRequestResult>, AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => Ok(runtime.apply_requests()?),
            SonaraBackend::Firewheel(backend) => Ok(backend.apply_requests()?),
        }
    }

    /// 依次执行所有待处理请求, 单条失败不会中断整批处理
    pub fn apply_requests_isolated(&mut self) -> Vec<AudioRequestOutcome> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime
                .apply_requests_isolated()
                .into_iter()
                .map(|outcome| AudioRequestOutcome {
                    request: outcome.request,
                    result: outcome.result.map_err(Into::into),
                })
                .collect(),
            SonaraBackend::Firewheel(backend) => backend
                .apply_requests_isolated()
                .into_iter()
                .map(|outcome| AudioRequestOutcome {
                    request: outcome.request,
                    result: outcome.result.map_err(Into::into),
                })
                .collect(),
        }
    }

    /// 停止一个事件实例
    pub fn stop(
        &mut self,
        instance_id: EventInstanceId,
        fade: Fade,
    ) -> Result<(), AudioBackendError> {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.stop(instance_id, fade)?,
            SonaraBackend::Firewheel(backend) => backend.stop(instance_id, fade)?,
        }

        Ok(())
    }

    /// 压入一个 snapshot
    pub fn push_snapshot(
        &mut self,
        snapshot_id: SnapshotId,
        fade: Fade,
    ) -> Result<SnapshotInstanceId, AudioBackendError> {
        Ok(self.runtime_mut().push_snapshot(snapshot_id, fade)?)
    }

    fn runtime(&self) -> &SonaraRuntime {
        match &self.backend {
            SonaraBackend::Runtime(runtime) => runtime.runtime(),
            SonaraBackend::Firewheel(backend) => backend.runtime(),
        }
    }

    fn runtime_mut(&mut self) -> &mut SonaraRuntime {
        match &mut self.backend {
            SonaraBackend::Runtime(runtime) => runtime.runtime_mut(),
            SonaraBackend::Firewheel(backend) => backend.runtime_mut(),
        }
    }
}

impl AudioUpdate<'_> {
    /// 确保一个发声体已经绑定到底层 runtime emitter。
    pub fn ensure_emitter(&mut self, emitter: &mut AudioEmitter) -> EmitterId {
        self.audio.ensure_emitter(emitter)
    }

    /// 释放一个发声体已绑定的 runtime emitter。
    pub fn detach_emitter(&mut self, emitter: &mut AudioEmitter) -> Result<(), AudioBackendError> {
        self.audio.detach_emitter(emitter)
    }

    /// 在这一帧里排队一个全局播放请求。
    pub fn play(&mut self, event_id: EventId) {
        self.audio.queue_play(event_id);
    }

    /// 在这一帧里排队一个 emitter 播放请求。
    pub fn play_from_emitter(&mut self, emitter: &mut AudioEmitter, event_id: EventId) {
        self.audio.queue_play_from_emitter(emitter, event_id);
    }

    /// 在这一帧里排队一个全局参数更新。
    pub fn set_global_param(&mut self, parameter_id: ParameterId, value: ParameterValue) {
        self.audio.queue_set_global_param(parameter_id, value);
    }

    /// 在这一帧里排队一个 emitter 参数更新。
    pub fn set_emitter_param_on(
        &mut self,
        emitter: &mut AudioEmitter,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) {
        self.audio
            .queue_set_emitter_param_on(emitter, parameter_id, value);
    }

    /// 在这一帧里排队一个实例停止请求。
    pub fn stop(&mut self, instance_id: EventInstanceId, fade: Fade) {
        self.audio.queue_stop(instance_id, fade);
    }

    /// 统一执行这一帧收集到的请求。
    pub fn apply(self) -> Result<Vec<AudioRequestResult>, AudioBackendError> {
        self.audio.apply_requests()
    }

    /// 统一执行这一帧收集到的请求, 单条失败不会中断整批。
    pub fn apply_isolated(self) -> Vec<AudioRequestOutcome> {
        self.audio.apply_requests_isolated()
    }
}

pub(crate) fn update_firewheel_backend_system(mut audio: NonSendMut<SonaraAudio>) {
    audio
        .update_backend()
        .expect("Firewheel backend update should succeed");
}
