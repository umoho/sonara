// SPDX-License-Identifier: MPL-2.0

use sonara_model::{
    Bank, BankId, Bus, BusId, Clip, Event, EventId, MusicGraph, MusicGraphId, MusicNodeId,
    ParameterId, ParameterValue, ResumeSlot, Snapshot, SnapshotId, SyncDomain, TrackGroupId,
};

use crate::bank::SonaraRuntime;
use crate::error::RuntimeError;
use crate::ids::{EmitterId, EventInstanceId, MusicSessionId, SnapshotInstanceId};
use crate::types::{EventInstanceState, Fade, MusicStatus, PlaybackPlan, TrackGroupState};

/// 运行时可消费的一条最小请求
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeRequest {
    Play {
        event_id: EventId,
    },
    PlayOnEmitter {
        emitter_id: EmitterId,
        event_id: EventId,
    },
    SetGlobalParam {
        parameter_id: ParameterId,
        value: ParameterValue,
    },
    SetEmitterParam {
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    },
    Stop {
        instance_id: EventInstanceId,
        fade: Fade,
    },
}

/// 运行时执行请求后的结果
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeRequestResult {
    Played { instance_id: EventInstanceId },
    Stopped { instance_id: EventInstanceId },
    ParameterSet,
}

/// 默认使用的运行时命令缓冲区类型
pub type RuntimeCommandBuffer = AudioCommandBuffer<RuntimeRequest>;

/// 带请求队列的纯 runtime 前端。
///
/// 这个类型适合不直接绑定真实音频后端, 但又希望复用统一请求模型的调用方。
#[derive(Debug, Default)]
pub struct QueuedRuntime {
    runtime: SonaraRuntime,
    command_buffer: RuntimeCommandBuffer,
}

/// 一组待执行的音频请求缓冲区
#[derive(Debug)]
pub struct AudioCommandBuffer<Request> {
    requests: Vec<Request>,
}

impl<Request> Default for AudioCommandBuffer<Request> {
    fn default() -> Self {
        Self::new()
    }
}

impl QueuedRuntime {
    /// 创建一个空的 queued runtime。
    pub fn new() -> Self {
        Self::default()
    }

    /// 读取内部 runtime。
    pub fn runtime(&self) -> &SonaraRuntime {
        &self.runtime
    }

    /// 读取内部 runtime 的可变引用。
    pub fn runtime_mut(&mut self) -> &mut SonaraRuntime {
        &mut self.runtime
    }

    /// 加载一个 bank。
    pub fn load_bank(&mut self, bank: Bank, events: Vec<Event>) -> Result<BankId, RuntimeError> {
        self.runtime.load_bank(bank, events)
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
    ) -> Result<BankId, RuntimeError> {
        self.runtime.load_bank_with_definitions(
            bank,
            events,
            buses,
            snapshots,
            clips,
            resume_slots,
            sync_domains,
            music_graphs,
        )
    }

    /// 播放一个未绑定 emitter 的事件。
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, RuntimeError> {
        self.runtime.play(event_id)
    }

    /// 在指定 emitter 上播放事件。
    pub fn play_on(
        &mut self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<EventInstanceId, RuntimeError> {
        self.runtime.play_on(emitter_id, event_id)
    }

    /// 设置一个全局参数。
    pub fn set_global_param(
        &mut self,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        self.runtime.set_global_param(parameter_id, value)
    }

    /// 设置一个 emitter 参数。
    pub fn set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        self.runtime
            .set_emitter_param(emitter_id, parameter_id, value)
    }

    /// 停止一个事件实例。
    pub fn stop(&mut self, instance_id: EventInstanceId, fade: Fade) -> Result<(), RuntimeError> {
        self.runtime.stop(instance_id, fade)
    }

    /// 压入一个 snapshot。
    pub fn push_snapshot(
        &mut self,
        snapshot_id: SnapshotId,
        fade: Fade,
    ) -> Result<SnapshotInstanceId, RuntimeError> {
        self.runtime.push_snapshot(snapshot_id, fade)
    }

    /// 设置一个 bus gain。
    pub fn set_bus_gain(&mut self, bus_id: BusId, gain: f32) -> Result<(), RuntimeError> {
        self.runtime.set_bus_gain(bus_id, gain)
    }

    /// 创建一个 emitter。
    pub fn create_emitter(&mut self) -> EmitterId {
        self.runtime.create_emitter()
    }

    /// 删除一个 emitter。
    pub fn remove_emitter(&mut self, emitter_id: EmitterId) -> Result<(), RuntimeError> {
        self.runtime.remove_emitter(emitter_id)
    }

    /// 读取当前活动实例的播放计划。
    pub fn active_plan(&self, instance_id: EventInstanceId) -> Option<&PlaybackPlan> {
        self.runtime.active_plan(instance_id)
    }

    /// 读取某个 bus 当前的 live gain。
    pub fn bus_gain(&self, bus_id: BusId) -> Option<f32> {
        self.runtime.bus_gain(bus_id)
    }

    /// 查询一个事件实例当前对游戏侧可见的播放状态。
    pub fn instance_state(&self, instance_id: EventInstanceId) -> EventInstanceState {
        self.runtime.instance_state(instance_id)
    }

    /// 启动一个音乐图会话。
    pub fn play_music_graph(
        &mut self,
        graph_id: MusicGraphId,
    ) -> Result<MusicSessionId, RuntimeError> {
        self.runtime.play_music_graph(graph_id)
    }

    /// 启动一个音乐图会话，并显式指定初始节点。
    pub fn play_music_graph_in_node(
        &mut self,
        graph_id: MusicGraphId,
        initial_node: Option<MusicNodeId>,
    ) -> Result<MusicSessionId, RuntimeError> {
        self.runtime
            .play_music_graph_in_node(graph_id, initial_node)
    }

    /// 请求一个音乐会话切换到目标节点。
    pub fn request_music_node(
        &mut self,
        session_id: MusicSessionId,
        target_node: MusicNodeId,
    ) -> Result<(), RuntimeError> {
        self.runtime.request_music_node(session_id, target_node)
    }

    /// 通知运行时：会话已到达允许退出的切点。
    pub fn complete_music_exit(&mut self, session_id: MusicSessionId) -> Result<(), RuntimeError> {
        self.runtime.complete_music_exit(session_id)
    }

    /// 通知运行时：桥接片段已经结束。
    pub fn complete_music_node_completion(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<(), RuntimeError> {
        self.runtime.complete_music_node_completion(session_id)
    }

    /// 停止一个音乐会话。
    pub fn stop_music_session(
        &mut self,
        session_id: MusicSessionId,
        fade: Fade,
    ) -> Result<(), RuntimeError> {
        self.runtime.stop_music_session(session_id, fade)
    }

    /// 查询音乐会话当前状态。
    pub fn music_status(&self, session_id: MusicSessionId) -> Result<MusicStatus, RuntimeError> {
        self.runtime.music_status(session_id)
    }

    /// 查询一个音乐会话中某个显式 track group 的当前状态。
    pub fn music_track_group_state(
        &self,
        session_id: MusicSessionId,
        group_id: TrackGroupId,
    ) -> Result<TrackGroupState, RuntimeError> {
        self.runtime.music_track_group_state(session_id, group_id)
    }

    /// 设置一个音乐会话中某个显式 track group 的开关状态。
    pub fn set_music_track_group_active(
        &mut self,
        session_id: MusicSessionId,
        group_id: TrackGroupId,
        active: bool,
    ) -> Result<(), RuntimeError> {
        self.runtime
            .set_music_track_group_active(session_id, group_id, active)
    }

    /// 取出当前待处理请求。
    pub fn drain_requests(&mut self) -> Vec<RuntimeRequest> {
        self.command_buffer.drain()
    }

    /// 排队一个播放请求。
    pub fn queue_play(&mut self, event_id: EventId) {
        self.command_buffer.queue_play(event_id);
    }

    /// 排队一个 emitter 播放请求。
    pub fn queue_play_on(&mut self, emitter_id: EmitterId, event_id: EventId) {
        self.command_buffer.queue_play_on(emitter_id, event_id);
    }

    /// 排队一个全局参数更新请求。
    pub fn queue_set_global_param(&mut self, parameter_id: ParameterId, value: ParameterValue) {
        self.command_buffer
            .queue_set_global_param(parameter_id, value);
    }

    /// 排队一个 emitter 参数更新请求。
    pub fn queue_set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) {
        self.command_buffer
            .queue_set_emitter_param(emitter_id, parameter_id, value);
    }

    /// 排队一个停止请求。
    pub fn queue_stop(&mut self, instance_id: EventInstanceId, fade: Fade) {
        self.command_buffer.queue_stop(instance_id, fade);
    }

    /// 执行所有已排队请求。
    pub fn apply_requests(&mut self) -> Result<Vec<RuntimeRequestResult>, RuntimeError> {
        self.command_buffer
            .apply(|request| self.runtime.apply_request(request))
    }

    /// 以隔离模式执行所有已排队请求。
    pub fn apply_requests_isolated(
        &mut self,
    ) -> Vec<AudioCommandOutcome<RuntimeRequest, RuntimeRequestResult, RuntimeError>> {
        self.command_buffer
            .apply_isolated(|request| self.runtime.apply_request(request))
    }
}

impl<Request> AudioCommandBuffer<Request> {
    /// 创建一个空缓冲区
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
        }
    }

    /// 追加一条请求
    pub fn push(&mut self, request: Request) {
        self.requests.push(request);
    }

    /// 取出当前所有待处理请求
    pub fn drain(&mut self) -> Vec<Request> {
        self.requests.drain(..).collect()
    }

    /// 当前缓冲区里的请求数量
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// 当前缓冲区是否为空
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// 依次执行所有待处理请求, 遇到第一条错误立即返回
    pub fn apply<Output, Error, Executor>(
        &mut self,
        mut executor: Executor,
    ) -> Result<Vec<Output>, Error>
    where
        Executor: FnMut(&Request) -> Result<Output, Error>,
    {
        let requests = self.drain();
        let mut results = Vec::with_capacity(requests.len());

        for request in requests {
            results.push(executor(&request)?);
        }

        Ok(results)
    }

    /// 依次执行所有待处理请求, 单条失败不会中断整批处理
    pub fn apply_isolated<Output, Error, Executor>(
        &mut self,
        mut executor: Executor,
    ) -> Vec<AudioCommandOutcome<Request, Output, Error>>
    where
        Executor: FnMut(&Request) -> Result<Output, Error>,
    {
        self.drain()
            .into_iter()
            .map(|request| {
                let result = executor(&request);
                AudioCommandOutcome { request, result }
            })
            .collect()
    }
}

impl RuntimeRequest {
    /// 构造一个未绑定 emitter 的播放请求
    pub fn play(event_id: EventId) -> Self {
        Self::Play { event_id }
    }

    /// 构造一个面向指定 emitter 的播放请求
    pub fn play_on(emitter_id: EmitterId, event_id: EventId) -> Self {
        Self::PlayOnEmitter {
            emitter_id,
            event_id,
        }
    }

    /// 构造一个全局参数更新请求
    pub fn set_global_param(parameter_id: ParameterId, value: ParameterValue) -> Self {
        Self::SetGlobalParam {
            parameter_id,
            value,
        }
    }

    /// 构造一个 emitter 参数更新请求
    pub fn set_emitter_param(
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Self {
        Self::SetEmitterParam {
            emitter_id,
            parameter_id,
            value,
        }
    }

    /// 构造一个停止实例请求
    pub fn stop(instance_id: EventInstanceId, fade: Fade) -> Self {
        Self::Stop { instance_id, fade }
    }
}

impl AudioCommandBuffer<RuntimeRequest> {
    /// 排队一个未绑定 emitter 的播放请求
    pub fn queue_play(&mut self, event_id: EventId) {
        self.push(RuntimeRequest::play(event_id));
    }

    /// 排队一个面向指定 emitter 的播放请求
    pub fn queue_play_on(&mut self, emitter_id: EmitterId, event_id: EventId) {
        self.push(RuntimeRequest::play_on(emitter_id, event_id));
    }

    /// 排队一个全局参数更新请求
    pub fn queue_set_global_param(&mut self, parameter_id: ParameterId, value: ParameterValue) {
        self.push(RuntimeRequest::set_global_param(parameter_id, value));
    }

    /// 排队一个 emitter 参数更新请求
    pub fn queue_set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) {
        self.push(RuntimeRequest::set_emitter_param(
            emitter_id,
            parameter_id,
            value,
        ));
    }

    /// 排队一个停止实例请求
    pub fn queue_stop(&mut self, instance_id: EventInstanceId, fade: Fade) {
        self.push(RuntimeRequest::stop(instance_id, fade));
    }
}

/// 一条请求在隔离执行模式下的结果
#[derive(Debug)]
pub struct AudioCommandOutcome<Request, Output, Error> {
    pub request: Request,
    pub result: Result<Output, Error>,
}

impl SonaraRuntime {
    /// 执行一条最小运行时请求
    pub fn apply_request(
        &mut self,
        request: &RuntimeRequest,
    ) -> Result<RuntimeRequestResult, RuntimeError> {
        match request {
            RuntimeRequest::Play { event_id } => Ok(RuntimeRequestResult::Played {
                instance_id: self.play(*event_id)?,
            }),
            RuntimeRequest::PlayOnEmitter {
                emitter_id,
                event_id,
            } => Ok(RuntimeRequestResult::Played {
                instance_id: self.play_on(*emitter_id, *event_id)?,
            }),
            RuntimeRequest::SetGlobalParam {
                parameter_id,
                value,
            } => {
                self.set_global_param(*parameter_id, value.clone())?;
                Ok(RuntimeRequestResult::ParameterSet)
            }
            RuntimeRequest::SetEmitterParam {
                emitter_id,
                parameter_id,
                value,
            } => {
                self.set_emitter_param(*emitter_id, *parameter_id, value.clone())?;
                Ok(RuntimeRequestResult::ParameterSet)
            }
            RuntimeRequest::Stop { instance_id, fade } => {
                self.stop(*instance_id, *fade)?;
                Ok(RuntimeRequestResult::Stopped {
                    instance_id: *instance_id,
                })
            }
        }
    }
}
