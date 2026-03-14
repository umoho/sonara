//! Sonara 的高层运行时接口

use std::collections::HashMap;

use sonara_model::{
    Bank, BankId, BankObjects, BusId, Clip, ClipId, EdgeTrigger, EntryPolicy, Event,
    EventContentNode, EventId, ExitPolicy, MusicGraph, MusicGraphId, MusicStateId, MusicStateNode,
    NodeId, NodeRef, ParameterId, ParameterValue, PlaybackTarget, ResumeSlot, ResumeSlotId,
    Snapshot, SnapshotId, SyncDomain, SyncDomainId, TrackId, TransitionRule,
};
use thiserror::Error;
use uuid::Uuid;

/// 运行时事件实例 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventInstanceId(u64);

/// 运行时 snapshot 实例 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotInstanceId(u64);

/// 运行时音乐会话 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MusicSessionId(u64);

/// 运行时 emitter ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EmitterId(u64);

/// 停止或切换时使用的淡变参数
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fade {
    pub duration_seconds: f32,
}

impl Fade {
    /// 立即切换, 不做淡变
    pub const IMMEDIATE: Self = Self {
        duration_seconds: 0.0,
    };

    /// 使用秒数构造淡变
    pub fn seconds(duration_seconds: f32) -> Self {
        Self { duration_seconds }
    }
}

/// 一次事件触发后得到的最小播放计划
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackPlan {
    pub event_id: EventId,
    pub emitter_id: Option<EmitterId>,
    pub asset_ids: Vec<Uuid>,
}

/// 事件实例当前对游戏侧可见的播放状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventInstanceState {
    /// 实例已经建立, 但媒体还没准备到可实际发声。
    PendingMedia,
    /// 实例已经进入实际播放。
    Playing,
    /// 实例不存在或已经停止。
    Stopped,
}

/// 运行中的事件实例
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEventInstance {
    pub id: EventInstanceId,
    pub event_id: EventId,
    pub emitter_id: Option<EmitterId>,
    pub plan: PlaybackPlan,
}

/// 运行中的 snapshot 实例
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveSnapshotInstance {
    pub id: SnapshotInstanceId,
    pub snapshot_id: SnapshotId,
    pub fade: Fade,
    pub overrides: HashMap<BusId, f32>,
}

/// 音乐会话当前所处的逻辑阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicPhase {
    Stable,
    WaitingExitCue,
    WaitingNodeCompletion,
    EnteringDestination,
    Stopped,
}

/// 一条等待完成的音乐状态切换。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMusicTransition {
    pub from_node: MusicStateId,
    pub to_node: MusicStateId,
    pub requested_target_node: MusicStateId,
    pub trigger: EdgeTrigger,
    pub destination: EntryPolicy,
}

/// 运行中的音乐会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMusicSession {
    pub id: MusicSessionId,
    pub graph_id: MusicGraphId,
    pub desired_target_node: MusicStateId,
    pub active_node: MusicStateId,
    pub current_entry: EntryPolicy,
    pub phase: MusicPhase,
    pub pending_transition: Option<PendingMusicTransition>,
}

/// 对游戏逻辑暴露的音乐会话状态快照。
#[derive(Debug, Clone, PartialEq)]
pub struct MusicStatus {
    pub session_id: MusicSessionId,
    pub graph_id: MusicGraphId,
    pub desired_target_node: MusicStateId,
    pub active_node: MusicStateId,
    pub phase: MusicPhase,
    pub current_track_id: Option<TrackId>,
    pub current_target: PlaybackTarget,
    pub pending_transition: Option<PendingMusicTransition>,
}

/// 一个记忆槽当前保存的播放头。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResumeMemoryEntry {
    pub position_seconds: f64,
    pub saved_at_seconds: f64,
}

/// 运行时为当前音乐会话解析出的播放目标。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMusicPlayback {
    pub clip_id: ClipId,
    pub track_id: Option<TrackId>,
    pub entry_offset_seconds: f64,
}

/// 一次“从当前位置往后找最近匹配 cue”后的解析结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NextCueMatch {
    pub cue_position_seconds: f64,
    pub requires_wrap: bool,
}

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
        buses: Vec<sonara_model::Bus>,
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

    /// 启动一个音乐图会话，并显式指定初始状态。
    pub fn play_music_graph_in_state(
        &mut self,
        graph_id: MusicGraphId,
        initial_state: Option<MusicStateId>,
    ) -> Result<MusicSessionId, RuntimeError> {
        self.runtime
            .play_music_graph_in_state(graph_id, initial_state)
    }

    /// 请求一个音乐会话切换到目标状态。
    pub fn request_music_state(
        &mut self,
        session_id: MusicSessionId,
        target_state: MusicStateId,
    ) -> Result<(), RuntimeError> {
        self.runtime.request_music_state(session_id, target_state)
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

/// 运行时错误
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("event `{0:?}` is not loaded")]
    EventNotLoaded(EventId),
    #[error("bank `{0:?}` is not loaded")]
    BankNotLoaded(BankId),
    #[error("parameter `{0:?}` is not available")]
    ParameterUnavailable(ParameterId),
    #[error("switch parameter `{0:?}` is not an enum value")]
    SwitchParameterTypeMismatch(ParameterId),
    #[error("switch parameter `{0:?}` 没有匹配分支")]
    NoMatchingSwitchCase(ParameterId),
    #[error("事件内容树中缺少节点 `{0:?}`")]
    MissingNode(NodeId),
    #[error("事件实例 `{0:?}` 不存在")]
    EventInstanceNotFound(EventInstanceId),
    #[error("emitter `{0:?}` 不存在")]
    EmitterNotFound(EmitterId),
    #[error("snapshot `{0:?}` 不存在")]
    SnapshotNotLoaded(SnapshotId),
    #[error("snapshot 引用了不存在的 bus `{0:?}`")]
    SnapshotTargetBusNotFound(BusId),
    #[error("music graph `{0:?}` is not loaded")]
    MusicGraphNotLoaded(MusicGraphId),
    #[error("music graph `{0:?}` has no states")]
    MusicGraphHasNoStates(MusicGraphId),
    #[error("music graph `{graph_id:?}` has no state `{state_id:?}`")]
    MusicStateNotFound {
        graph_id: MusicGraphId,
        state_id: MusicStateId,
    },
    #[error("music session `{0:?}` 不存在")]
    MusicSessionNotFound(MusicSessionId),
    #[error("music graph `{graph_id:?}` has no transition `{from:?} -> {to:?}`")]
    MusicTransitionNotFound {
        graph_id: MusicGraphId,
        from: MusicStateId,
        to: MusicStateId,
    },
    #[error("music session `{session_id:?}` expected phase `{expected:?}`, got `{actual:?}`")]
    MusicSessionPhaseMismatch {
        session_id: MusicSessionId,
        expected: MusicPhase,
        actual: MusicPhase,
    },
    #[error("music session `{0:?}` has no pending transition")]
    MusicSessionHasNoPendingTransition(MusicSessionId),
}

/// 面向游戏逻辑的运行时入口
#[derive(Debug, Default)]
pub struct SonaraRuntime {
    banks: HashMap<BankId, BankObjects>,
    events: HashMap<EventId, Event>,
    clips: HashMap<ClipId, Clip>,
    resume_slots: HashMap<ResumeSlotId, ResumeSlot>,
    sync_domains: HashMap<SyncDomainId, SyncDomain>,
    music_graphs: HashMap<MusicGraphId, MusicGraph>,
    snapshots: HashMap<SnapshotId, Snapshot>,
    bus_volumes: HashMap<BusId, f32>,
    global_parameters: HashMap<ParameterId, ParameterValue>,
    emitter_parameters: HashMap<EmitterId, HashMap<ParameterId, ParameterValue>>,
    active_instances: HashMap<EventInstanceId, ActiveEventInstance>,
    music_sessions: HashMap<MusicSessionId, ActiveMusicSession>,
    resume_memories: HashMap<ResumeSlotId, ResumeMemoryEntry>,
    active_snapshots: HashMap<SnapshotInstanceId, ActiveSnapshotInstance>,
    next_event_instance_id: u64,
    next_music_session_id: u64,
    next_snapshot_instance_id: u64,
    next_emitter_id: u64,
}

impl SonaraRuntime {
    /// 创建一个空运行时
    pub fn new() -> Self {
        Self::default()
    }

    /// 加载一个 bank 和它包含的事件定义
    pub fn load_bank(&mut self, bank: Bank, events: Vec<Event>) -> Result<BankId, RuntimeError> {
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
        buses: Vec<sonara_model::Bus>,
        snapshots: Vec<Snapshot>,
        clips: Vec<Clip>,
        resume_slots: Vec<ResumeSlot>,
        sync_domains: Vec<SyncDomain>,
        music_graphs: Vec<MusicGraph>,
    ) -> Result<BankId, RuntimeError> {
        let bank_id = bank.id;
        let bank_objects = bank.objects;

        for event in events {
            self.events.insert(event.id, event);
        }

        for bus in buses {
            self.bus_volumes.entry(bus.id).or_insert(bus.default_volume);
        }

        for bus_id in &bank_objects.buses {
            self.bus_volumes.entry(*bus_id).or_insert(1.0);
        }

        for snapshot in snapshots {
            self.snapshots.insert(snapshot.id, snapshot);
        }

        for clip in clips {
            self.clips.insert(clip.id, clip);
        }

        for resume_slot in resume_slots {
            self.resume_slots.insert(resume_slot.id, resume_slot);
        }

        for sync_domain in sync_domains {
            self.sync_domains.insert(sync_domain.id, sync_domain);
        }

        for music_graph in music_graphs {
            self.music_graphs.insert(music_graph.id, music_graph);
        }

        self.banks.insert(bank_id, bank_objects);

        Ok(bank_id)
    }

    /// 卸载一个 bank
    pub fn unload_bank(&mut self, bank_id: BankId) -> Result<(), RuntimeError> {
        let bank = self
            .banks
            .remove(&bank_id)
            .ok_or(RuntimeError::BankNotLoaded(bank_id))?;
        let event_ids = bank.events.clone();

        for event_id in &event_ids {
            self.events.remove(event_id);
        }

        for clip_id in &bank.clips {
            self.clips.remove(clip_id);
        }

        for resume_slot_id in &bank.resume_slots {
            self.resume_slots.remove(resume_slot_id);
            self.resume_memories.remove(resume_slot_id);
        }

        for sync_domain_id in &bank.sync_domains {
            self.sync_domains.remove(sync_domain_id);
        }

        for music_graph_id in &bank.music_graphs {
            self.music_graphs.remove(music_graph_id);
        }

        self.active_instances
            .retain(|_, instance| !event_ids.contains(&instance.event_id));
        self.music_sessions
            .retain(|_, session| !bank.music_graphs.contains(&session.graph_id));

        Ok(())
    }

    /// 判断某个 bank 是否已加载
    pub fn is_bank_loaded(&self, bank_id: BankId) -> bool {
        self.banks.contains_key(&bank_id)
    }

    /// 读取某个已加载 bank 的对象清单。
    pub fn loaded_bank_objects(&self, bank_id: BankId) -> Option<&BankObjects> {
        self.banks.get(&bank_id)
    }

    /// 读取一个已加载的 clip 定义。
    pub fn clip(&self, clip_id: ClipId) -> Option<&Clip> {
        self.clips.get(&clip_id)
    }

    /// 读取一个已加载的记忆槽定义。
    pub fn resume_slot(&self, resume_slot_id: ResumeSlotId) -> Option<&ResumeSlot> {
        self.resume_slots.get(&resume_slot_id)
    }

    /// 读取一个已加载的同步域定义。
    pub fn sync_domain(&self, sync_domain_id: SyncDomainId) -> Option<&SyncDomain> {
        self.sync_domains.get(&sync_domain_id)
    }

    /// 读取一个已加载的音乐图定义。
    pub fn music_graph(&self, music_graph_id: MusicGraphId) -> Option<&MusicGraph> {
        self.music_graphs.get(&music_graph_id)
    }

    /// 读取一个运行中的音乐会话。
    pub fn music_session(&self, session_id: MusicSessionId) -> Option<&ActiveMusicSession> {
        self.music_sessions.get(&session_id)
    }

    /// 读取一个记忆槽当前保存的播放头。
    pub fn resume_memory(&self, resume_slot_id: ResumeSlotId) -> Option<&ResumeMemoryEntry> {
        self.resume_memories.get(&resume_slot_id)
    }

    /// 启动一个音乐图会话，使用图中声明的初始状态。
    pub fn play_music_graph(
        &mut self,
        graph_id: MusicGraphId,
    ) -> Result<MusicSessionId, RuntimeError> {
        self.play_music_graph_in_state(graph_id, None)
    }

    /// 启动一个音乐图会话，并显式指定初始状态。
    pub fn play_music_graph_in_state(
        &mut self,
        graph_id: MusicGraphId,
        initial_state: Option<MusicStateId>,
    ) -> Result<MusicSessionId, RuntimeError> {
        let graph = self
            .music_graphs
            .get(&graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(graph_id))?;
        let active_node = resolve_music_graph_state(graph, initial_state)?;
        let state = lookup_music_node(graph, active_node)?;
        let session_id = MusicSessionId(self.next_music_session_id);
        self.next_music_session_id += 1;

        self.music_sessions.insert(
            session_id,
            ActiveMusicSession {
                id: session_id,
                graph_id,
                desired_target_node: active_node,
                active_node,
                current_entry: state.default_entry.clone(),
                phase: MusicPhase::Stable,
                pending_transition: None,
            },
        );

        Ok(session_id)
    }

    /// 请求一个音乐会话切换到目标状态。
    pub fn request_music_state(
        &mut self,
        session_id: MusicSessionId,
        target_state: MusicStateId,
    ) -> Result<(), RuntimeError> {
        let (graph_id, active_node, phase) = {
            let session = self
                .music_sessions
                .get(&session_id)
                .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
            (session.graph_id, session.active_node, session.phase)
        };

        if phase == MusicPhase::Stopped {
            return Err(RuntimeError::MusicSessionPhaseMismatch {
                session_id,
                expected: MusicPhase::Stable,
                actual: phase,
            });
        }

        let graph = self
            .music_graphs
            .get(&graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(graph_id))?;
        let target_node = lookup_music_node(graph, target_state)?;
        if !target_node.externally_targetable {
            return Err(RuntimeError::MusicTransitionNotFound {
                graph_id,
                from: active_node,
                to: target_state,
            });
        }

        if active_node == target_state {
            let session = self
                .music_sessions
                .get_mut(&session_id)
                .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
            session.desired_target_node = target_state;
            session.phase = MusicPhase::Stable;
            session.pending_transition = None;
            return Ok(());
        }

        let transition = lookup_transition_rule(graph, active_node, target_state)?.clone();
        let pending_transition =
            Self::build_pending_transition(active_node, target_state, &transition);
        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        session.desired_target_node = target_state;

        if matches!(transition.trigger, EdgeTrigger::Immediate) {
            self.enter_music_node(
                session_id,
                transition.to,
                target_state,
                transition.destination,
            )?;
            return Ok(());
        }

        session.pending_transition = Some(pending_transition);
        session.phase = MusicPhase::WaitingExitCue;

        Ok(())
    }

    /// 预览一次音乐状态切换将使用的最小 transition 语义。
    pub fn preview_music_transition(
        &self,
        session_id: MusicSessionId,
        target_state: MusicStateId,
    ) -> Result<Option<PendingMusicTransition>, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        if session.phase == MusicPhase::Stopped {
            return Err(RuntimeError::MusicSessionPhaseMismatch {
                session_id,
                expected: MusicPhase::Stable,
                actual: session.phase,
            });
        }

        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        lookup_music_node(graph, target_state)?;

        if session.active_node == target_state {
            return Ok(None);
        }

        let transition = lookup_transition_rule(graph, session.active_node, target_state)?;
        Ok(Some(Self::build_pending_transition(
            session.active_node,
            target_state,
            transition,
        )))
    }

    fn build_pending_transition(
        from_node: MusicStateId,
        requested_target_node: MusicStateId,
        transition: &TransitionRule,
    ) -> PendingMusicTransition {
        PendingMusicTransition {
            from_node,
            to_node: transition.to,
            requested_target_node,
            trigger: transition.trigger.clone(),
            destination: transition.destination.clone(),
        }
    }

    fn enter_music_node(
        &mut self,
        session_id: MusicSessionId,
        node_id: MusicStateId,
        requested_target_node: MusicStateId,
        entry_policy: EntryPolicy,
    ) -> Result<(), RuntimeError> {
        let (graph_id, next_edge) = {
            let session = self
                .music_sessions
                .get(&session_id)
                .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
            let graph = self
                .music_graphs
                .get(&session.graph_id)
                .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
            lookup_music_node(graph, node_id)?;
            (
                session.graph_id,
                lookup_auto_transition_rule(graph, node_id, requested_target_node).cloned(),
            )
        };

        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        session.active_node = node_id;
        session.current_entry = entry_policy;
        session.desired_target_node = requested_target_node;

        if let Some(edge) = next_edge {
            session.pending_transition = Some(Self::build_pending_transition(
                node_id,
                requested_target_node,
                &edge,
            ));
            session.phase = MusicPhase::WaitingNodeCompletion;
        } else {
            session.phase = MusicPhase::Stable;
            session.pending_transition = None;
        }

        let _ = graph_id;
        Ok(())
    }

    /// 通知运行时：当前会话已到达允许退出的切点。
    pub fn complete_music_exit(&mut self, session_id: MusicSessionId) -> Result<(), RuntimeError> {
        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;

        if session.phase != MusicPhase::WaitingExitCue {
            return Err(RuntimeError::MusicSessionPhaseMismatch {
                session_id,
                expected: MusicPhase::WaitingExitCue,
                actual: session.phase,
            });
        }

        let pending = session
            .pending_transition
            .clone()
            .ok_or(RuntimeError::MusicSessionHasNoPendingTransition(session_id))?;

        let to_node = pending.to_node;
        let requested_target_node = pending.requested_target_node;
        let destination = pending.destination.clone();
        let _ = session;

        self.enter_music_node(session_id, to_node, requested_target_node, destination)
    }

    /// 通知运行时：当前自动推进节点已经完成，可以进入目标节点。
    pub fn complete_music_node_completion(
        &mut self,
        session_id: MusicSessionId,
    ) -> Result<(), RuntimeError> {
        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;

        if session.phase != MusicPhase::WaitingNodeCompletion {
            return Err(RuntimeError::MusicSessionPhaseMismatch {
                session_id,
                expected: MusicPhase::WaitingNodeCompletion,
                actual: session.phase,
            });
        }

        let pending = session
            .pending_transition
            .clone()
            .ok_or(RuntimeError::MusicSessionHasNoPendingTransition(session_id))?;

        let to_node = pending.to_node;
        let requested_target_node = pending.requested_target_node;
        let destination = pending.destination.clone();
        let _ = session;

        self.enter_music_node(session_id, to_node, requested_target_node, destination)
    }

    /// 停止一个音乐会话。
    pub fn stop_music_session(
        &mut self,
        session_id: MusicSessionId,
        _fade: Fade,
    ) -> Result<(), RuntimeError> {
        let session = self
            .music_sessions
            .get_mut(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        session.phase = MusicPhase::Stopped;
        session.pending_transition = None;
        Ok(())
    }

    /// 读取音乐会话当前对游戏侧可见的状态。
    pub fn music_status(&self, session_id: MusicSessionId) -> Result<MusicStatus, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let current_track_id = state.primary_binding(graph).map(|binding| binding.track_id);

        Ok(MusicStatus {
            session_id,
            graph_id: session.graph_id,
            desired_target_node: session.desired_target_node,
            active_node: session.active_node,
            phase: session.phase,
            current_track_id,
            current_target: state.primary_target(graph).cloned().ok_or(
                RuntimeError::MusicStateNotFound {
                    graph_id: graph.id,
                    state_id: session.active_node,
                },
            )?,
            pending_transition: session.pending_transition.clone(),
        })
    }

    /// 保存一个音乐会话当前状态对应的播放头到记忆槽。
    ///
    /// 只有当当前可听内容仍然对应 active state 时，才会写入记忆槽。
    pub fn save_music_session_resume_position(
        &mut self,
        session_id: MusicSessionId,
        position_seconds: f64,
        saved_at_seconds: f64,
    ) -> Result<bool, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;

        if !position_seconds.is_finite()
            || position_seconds < 0.0
            || !saved_at_seconds.is_finite()
            || saved_at_seconds < 0.0
        {
            return Ok(false);
        }

        if !matches!(
            session.phase,
            MusicPhase::Stable | MusicPhase::WaitingExitCue
        ) {
            return Ok(false);
        }

        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let Some(slot_id) = state.memory_slot else {
            return Ok(false);
        };

        self.resume_memories.insert(
            slot_id,
            ResumeMemoryEntry {
                position_seconds,
                saved_at_seconds,
            },
        );
        Ok(true)
    }

    /// 为当前音乐会话解析出真正应该播放的 clip 与入口偏移。
    pub fn resolve_music_playback(
        &self,
        session_id: MusicSessionId,
        now_seconds: f64,
    ) -> Result<ResolvedMusicPlayback, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let binding = state
            .primary_binding(graph)
            .ok_or(RuntimeError::MusicStateNotFound {
                graph_id: graph.id,
                state_id: session.active_node,
            })?;
        let clip_id = match &binding.target {
            PlaybackTarget::Clip { clip_id } => clip_id,
        };
        let entry_offset_seconds =
            self.resolve_entry_offset_seconds(state, graph, &session.current_entry, now_seconds);

        Ok(ResolvedMusicPlayback {
            clip_id: *clip_id,
            track_id: Some(binding.track_id),
            entry_offset_seconds,
        })
    }

    /// 为当前活动节点解析一条 stinger track 播放目标。
    pub fn resolve_music_stinger_playback(
        &self,
        session_id: MusicSessionId,
    ) -> Result<Option<ResolvedMusicPlayback>, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let Some(binding) = state.binding_for_role(graph, sonara_model::TrackRole::Stinger) else {
            return Ok(None);
        };
        let clip_id = match &binding.target {
            PlaybackTarget::Clip { clip_id } => *clip_id,
        };

        Ok(Some(ResolvedMusicPlayback {
            clip_id,
            track_id: Some(binding.track_id),
            entry_offset_seconds: 0.0,
        }))
    }

    /// 为当前 waiting transition 解析下一个合法退出 cue。
    pub fn find_next_music_exit_cue(
        &self,
        session_id: MusicSessionId,
        current_position_seconds: f64,
    ) -> Result<Option<NextCueMatch>, RuntimeError> {
        let session = self
            .music_sessions
            .get(&session_id)
            .ok_or(RuntimeError::MusicSessionNotFound(session_id))?;
        let Some(pending) = &session.pending_transition else {
            return Ok(None);
        };
        let ExitPolicy::NextMatchingCue { tag } = &pending.trigger else {
            return Ok(None);
        };

        let graph = self
            .music_graphs
            .get(&session.graph_id)
            .ok_or(RuntimeError::MusicGraphNotLoaded(session.graph_id))?;
        let state = lookup_music_node(graph, session.active_node)?;
        let clip_id = match state
            .primary_target(graph)
            .ok_or(RuntimeError::MusicStateNotFound {
                graph_id: graph.id,
                state_id: session.active_node,
            })? {
            PlaybackTarget::Clip { clip_id } => clip_id,
        };
        let Some(clip) = self.clips.get(&clip_id) else {
            return Ok(None);
        };

        Ok(find_next_matching_cue_in_clip(
            clip,
            tag,
            current_position_seconds,
        ))
    }

    fn resolve_entry_offset_seconds(
        &self,
        state: &MusicStateNode,
        graph: &MusicGraph,
        entry_policy: &EntryPolicy,
        now_seconds: f64,
    ) -> f64 {
        match entry_policy {
            EntryPolicy::Resume => self
                .resolve_resume_offset_seconds(state, now_seconds)
                .unwrap_or_else(|| {
                    self.resolve_reset_entry_offset_seconds(state, graph, now_seconds)
                }),
            EntryPolicy::EntryCue { tag } => {
                self.resolve_entry_cue_offset_seconds(state, graph, tag)
            }
            EntryPolicy::ClipStart
            | EntryPolicy::ResumeNextMatchingCue { .. }
            | EntryPolicy::SameSyncPosition => 0.0,
        }
    }

    fn resolve_resume_offset_seconds(
        &self,
        state: &MusicStateNode,
        now_seconds: f64,
    ) -> Option<f64> {
        let slot_id = state.memory_slot?;
        let entry = self.resume_memories.get(&slot_id)?;
        let ttl_seconds = state
            .memory_policy
            .ttl_seconds
            .map(|ttl| ttl.max(0.0) as f64);

        if let Some(ttl_seconds) = ttl_seconds {
            if now_seconds.is_finite() && now_seconds >= 0.0 {
                let age_seconds = (now_seconds - entry.saved_at_seconds).max(0.0);
                if age_seconds > ttl_seconds {
                    return None;
                }
            }
        }

        Some(entry.position_seconds.max(0.0))
    }

    fn resolve_reset_entry_offset_seconds(
        &self,
        state: &MusicStateNode,
        graph: &MusicGraph,
        now_seconds: f64,
    ) -> f64 {
        match &state.memory_policy.reset_to {
            EntryPolicy::Resume => 0.0,
            EntryPolicy::ClipStart
            | EntryPolicy::ResumeNextMatchingCue { .. }
            | EntryPolicy::SameSyncPosition => {
                let _ = now_seconds;
                0.0
            }
            EntryPolicy::EntryCue { tag } => {
                self.resolve_entry_cue_offset_seconds(state, graph, tag)
            }
        }
    }

    fn resolve_entry_cue_offset_seconds(
        &self,
        state: &MusicStateNode,
        graph: &MusicGraph,
        tag: &str,
    ) -> f64 {
        let Some(target) = state.primary_target(graph) else {
            return 0.0;
        };
        let clip_id = match target {
            PlaybackTarget::Clip { clip_id } => clip_id,
        };
        let Some(clip) = self.clips.get(&clip_id) else {
            return 0.0;
        };

        clip.cues
            .iter()
            .filter(|cue| cue.tags.iter().any(|candidate| candidate.as_str() == tag))
            .map(|cue| cue.position_seconds.max(0.0) as f64)
            .min_by(|left, right| left.total_cmp(right))
            .unwrap_or(0.0)
    }

    /// 创建一个新的 emitter
    pub fn create_emitter(&mut self) -> EmitterId {
        let emitter_id = EmitterId(self.next_emitter_id);
        self.next_emitter_id += 1;
        self.emitter_parameters.insert(emitter_id, HashMap::new());
        emitter_id
    }

    /// 删除一个 emitter
    pub fn remove_emitter(&mut self, emitter_id: EmitterId) -> Result<(), RuntimeError> {
        self.emitter_parameters
            .remove(&emitter_id)
            .map(|_| ())
            .ok_or(RuntimeError::EmitterNotFound(emitter_id))
    }

    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, RuntimeError> {
        self.play_internal(event_id, None)
    }

    /// 在 emitter 上播放一个事件
    pub fn play_on(
        &mut self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<EventInstanceId, RuntimeError> {
        if !self.emitter_parameters.contains_key(&emitter_id) {
            return Err(RuntimeError::EmitterNotFound(emitter_id));
        }

        self.play_internal(event_id, Some(emitter_id))
    }

    /// 在不创建实例的情况下解析一个事件
    pub fn plan_event(&self, event_id: EventId) -> Result<PlaybackPlan, RuntimeError> {
        self.plan_event_for_emitter(None, event_id)
    }

    /// 在指定 emitter 上解析一个事件
    pub fn plan_event_on(
        &self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<PlaybackPlan, RuntimeError> {
        if !self.emitter_parameters.contains_key(&emitter_id) {
            return Err(RuntimeError::EmitterNotFound(emitter_id));
        }

        self.plan_event_for_emitter(Some(emitter_id), event_id)
    }

    /// 停止一个事件实例
    pub fn stop(&mut self, instance_id: EventInstanceId, _fade: Fade) -> Result<(), RuntimeError> {
        self.active_instances
            .remove(&instance_id)
            .map(|_| ())
            .ok_or(RuntimeError::EventInstanceNotFound(instance_id))
    }

    /// 获取事件实例的当前播放计划
    pub fn active_plan(&self, instance_id: EventInstanceId) -> Option<&PlaybackPlan> {
        self.active_instances
            .get(&instance_id)
            .map(|instance| &instance.plan)
    }

    /// 查询一个事件实例当前对游戏侧可见的播放状态。
    pub fn instance_state(&self, instance_id: EventInstanceId) -> EventInstanceState {
        if self.active_instances.contains_key(&instance_id) {
            EventInstanceState::Playing
        } else {
            EventInstanceState::Stopped
        }
    }

    /// 读取一个运行中的 snapshot 实例。
    pub fn active_snapshot(
        &self,
        instance_id: SnapshotInstanceId,
    ) -> Option<&ActiveSnapshotInstance> {
        self.active_snapshots.get(&instance_id)
    }

    /// 设置全局参数
    pub fn set_global_param(
        &mut self,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        self.global_parameters.insert(parameter_id, value);
        Ok(())
    }

    /// 读取一个全局参数
    pub fn global_param(&self, parameter_id: ParameterId) -> Option<&ParameterValue> {
        self.global_parameters.get(&parameter_id)
    }

    /// 加载一个 snapshot 定义。
    pub fn load_snapshot(&mut self, snapshot: Snapshot) {
        self.snapshots.insert(snapshot.id, snapshot);
    }

    /// 读取当前某个 bus 的目标音量。
    pub fn bus_volume(&self, bus_id: BusId) -> Option<f32> {
        self.bus_volumes.get(&bus_id).copied()
    }

    /// 读取某个事件实例当前命中的默认 bus 音量。
    ///
    /// 如果事件没有默认 bus，则返回 `1.0`。
    pub fn active_bus_volume(&self, instance_id: EventInstanceId) -> Option<f32> {
        let instance = self.active_instances.get(&instance_id)?;
        let event = self.events.get(&instance.event_id)?;

        Some(
            event
                .default_bus
                .and_then(|bus_id| self.bus_volume(bus_id))
                .unwrap_or(1.0),
        )
    }

    /// 设置 emitter 参数
    pub fn set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        let parameters = self
            .emitter_parameters
            .get_mut(&emitter_id)
            .ok_or(RuntimeError::EmitterNotFound(emitter_id))?;

        parameters.insert(parameter_id, value);
        Ok(())
    }

    /// 读取 emitter 参数
    pub fn emitter_param(
        &self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
    ) -> Option<&ParameterValue> {
        self.emitter_parameters
            .get(&emitter_id)
            .and_then(|parameters| parameters.get(&parameter_id))
    }

    /// 压入一个 snapshot
    pub fn push_snapshot(
        &mut self,
        snapshot_id: SnapshotId,
        fade: Fade,
    ) -> Result<SnapshotInstanceId, RuntimeError> {
        let snapshot = self
            .snapshots
            .get(&snapshot_id)
            .ok_or(RuntimeError::SnapshotNotLoaded(snapshot_id))?;
        let mut overrides = HashMap::with_capacity(snapshot.targets.len());

        for target in &snapshot.targets {
            if !self.bus_volumes.contains_key(&target.bus_id) {
                return Err(RuntimeError::SnapshotTargetBusNotFound(target.bus_id));
            }

            self.bus_volumes.insert(target.bus_id, target.target_volume);
            overrides.insert(target.bus_id, target.target_volume);
        }

        let instance_id = SnapshotInstanceId(self.next_snapshot_instance_id);
        self.next_snapshot_instance_id += 1;
        self.active_snapshots.insert(
            instance_id,
            ActiveSnapshotInstance {
                id: instance_id,
                snapshot_id,
                fade,
                overrides,
            },
        );

        Ok(instance_id)
    }

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

    fn play_internal(
        &mut self,
        event_id: EventId,
        emitter_id: Option<EmitterId>,
    ) -> Result<EventInstanceId, RuntimeError> {
        let plan = self.plan_event_for_emitter(emitter_id, event_id)?;
        let instance_id = EventInstanceId(self.next_event_instance_id);
        self.next_event_instance_id += 1;

        self.active_instances.insert(
            instance_id,
            ActiveEventInstance {
                id: instance_id,
                event_id,
                emitter_id,
                plan,
            },
        );

        Ok(instance_id)
    }

    fn plan_event_for_emitter(
        &self,
        emitter_id: Option<EmitterId>,
        event_id: EventId,
    ) -> Result<PlaybackPlan, RuntimeError> {
        let event = self
            .events
            .get(&event_id)
            .ok_or(RuntimeError::EventNotLoaded(event_id))?;

        let node_lookup: HashMap<NodeId, &EventContentNode> = event
            .root
            .nodes
            .iter()
            .map(|node| (node.id(), node))
            .collect();
        let mut asset_ids = Vec::new();

        self.resolve_node(&node_lookup, emitter_id, event.root.root, &mut asset_ids)?;

        Ok(PlaybackPlan {
            event_id,
            emitter_id,
            asset_ids,
        })
    }

    fn resolve_node(
        &self,
        node_lookup: &HashMap<NodeId, &EventContentNode>,
        emitter_id: Option<EmitterId>,
        node_ref: NodeRef,
        asset_ids: &mut Vec<Uuid>,
    ) -> Result<(), RuntimeError> {
        let node = node_lookup
            .get(&node_ref.id)
            .ok_or(RuntimeError::MissingNode(node_ref.id))?;

        match node {
            EventContentNode::Sampler(node) => {
                asset_ids.push(node.asset_id);
            }
            EventContentNode::Random(node) => {
                // v0 先固定选择第一个分支, 让规划结果可预测且方便测试
                if let Some(child) = node.children.first().copied() {
                    self.resolve_node(node_lookup, emitter_id, child, asset_ids)?;
                }
            }
            EventContentNode::Sequence(node) | EventContentNode::Layer(node) => {
                for child in &node.children {
                    self.resolve_node(node_lookup, emitter_id, *child, asset_ids)?;
                }
            }
            EventContentNode::Switch(node) => {
                let selected = self
                    .resolve_switch_target(emitter_id, node.parameter_id, node)
                    .and_then(|selected| {
                        selected.ok_or(RuntimeError::NoMatchingSwitchCase(node.parameter_id))
                    })?;

                self.resolve_node(node_lookup, emitter_id, selected, asset_ids)?;
            }
            EventContentNode::Loop(node) => {
                // v0 只为 loop 规划一次内容
                self.resolve_node(node_lookup, emitter_id, node.child, asset_ids)?;
            }
        }

        Ok(())
    }

    fn resolve_switch_target(
        &self,
        emitter_id: Option<EmitterId>,
        parameter_id: ParameterId,
        node: &sonara_model::SwitchNode,
    ) -> Result<Option<NodeRef>, RuntimeError> {
        let parameter_value = emitter_id
            .and_then(|emitter_id| self.emitter_param(emitter_id, parameter_id))
            .or_else(|| self.global_param(parameter_id));

        let selected = match parameter_value {
            Some(ParameterValue::Enum(variant)) => node
                .cases
                .iter()
                .find(|case| case.variant == *variant)
                .map(|case| case.child)
                .or(node.default_case),
            Some(_) => {
                return Err(RuntimeError::SwitchParameterTypeMismatch(parameter_id));
            }
            None => node.default_case,
        };

        Ok(selected)
    }
}

fn resolve_music_graph_state(
    graph: &MusicGraph,
    requested_state: Option<MusicStateId>,
) -> Result<MusicStateId, RuntimeError> {
    if let Some(state_id) = requested_state.or(graph.initial_node) {
        lookup_music_node(graph, state_id)?;
        return Ok(state_id);
    }

    graph
        .nodes
        .iter()
        .find(|node| node.externally_targetable)
        .or_else(|| graph.nodes.first())
        .map(|node| node.id)
        .ok_or(RuntimeError::MusicGraphHasNoStates(graph.id))
}

fn lookup_music_node(
    graph: &MusicGraph,
    node_id: MusicStateId,
) -> Result<&MusicStateNode, RuntimeError> {
    graph
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or(RuntimeError::MusicStateNotFound {
            graph_id: graph.id,
            state_id: node_id,
        })
}

fn lookup_transition_rule(
    graph: &MusicGraph,
    from: MusicStateId,
    requested_target_node: MusicStateId,
) -> Result<&TransitionRule, RuntimeError> {
    graph
        .edges
        .iter()
        .find(|edge| {
            edge.from == from
                && !matches!(edge.trigger, EdgeTrigger::OnComplete)
                && edge.requested_target.unwrap_or(edge.to) == requested_target_node
        })
        .ok_or(RuntimeError::MusicTransitionNotFound {
            graph_id: graph.id,
            from,
            to: requested_target_node,
        })
}

fn lookup_auto_transition_rule(
    graph: &MusicGraph,
    from: MusicStateId,
    requested_target_node: MusicStateId,
) -> Option<&TransitionRule> {
    graph.edges.iter().find(|edge| {
        edge.from == from
            && matches!(edge.trigger, EdgeTrigger::OnComplete)
            && edge
                .requested_target
                .map(|target| target == requested_target_node)
                .unwrap_or(true)
    })
}

fn find_next_matching_cue_in_clip(
    clip: &Clip,
    tag: &str,
    current_position_seconds: f64,
) -> Option<NextCueMatch> {
    let current_position_seconds = if current_position_seconds.is_finite() {
        current_position_seconds.max(0.0)
    } else {
        0.0
    };

    let mut matching_positions: Vec<f64> = clip
        .cues
        .iter()
        .filter(|cue| cue.tags.iter().any(|candidate| candidate.as_str() == tag))
        .map(|cue| cue.position_seconds.max(0.0) as f64)
        .collect();
    matching_positions.sort_by(|left, right| left.total_cmp(right));

    if let Some(position) = matching_positions
        .iter()
        .copied()
        .find(|position| *position >= current_position_seconds)
    {
        return Some(NextCueMatch {
            cue_position_seconds: position,
            requires_wrap: false,
        });
    }

    let first_position = matching_positions.first().copied()?;
    clip.loop_range.as_ref()?;

    Some(NextCueMatch {
        cue_position_seconds: first_position,
        requires_wrap: true,
    })
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use sonara_model::{
        CuePoint, EntryPolicy, EventContentRoot, EventKind, ExitPolicy, MemoryPolicy, MusicGraph,
        MusicStateId, MusicStateNode, PlaybackTarget, ResumeSlot, SamplerNode, SequenceNode,
        Snapshot, SnapshotTarget, SpatialMode, SwitchCase, SwitchNode, SyncDomain, TimeRange,
        Track, TrackBinding, TrackRole, TransitionRule,
    };

    use super::*;

    fn make_sampler(asset_id: Uuid) -> (NodeId, EventContentNode) {
        let id = NodeId::new();
        (id, EventContentNode::Sampler(SamplerNode { id, asset_id }))
    }

    fn make_event(id: EventId, root: NodeId, nodes: Vec<EventContentNode>) -> Event {
        Event {
            id,
            name: SmolStr::new("player.footstep"),
            kind: EventKind::OneShot,
            root: EventContentRoot {
                root: NodeRef { id: root },
                nodes,
            },
            default_bus: None,
            spatial: SpatialMode::ThreeD,
            default_parameters: Vec::new(),
            voice_limit: None,
            steal_policy: None,
        }
    }

    #[test]
    fn play_creates_an_active_instance_with_plan() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let (sampler_id, sampler) = make_sampler(asset_id);
        let event = make_event(event_id, sampler_id, vec![sampler]);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let instance_id = runtime.play(event_id).expect("event should play");

        assert_eq!(
            runtime.active_plan(instance_id),
            Some(&PlaybackPlan {
                event_id,
                emitter_id: None,
                asset_ids: vec![asset_id],
            })
        );
    }

    #[test]
    fn plan_event_resolves_switch_from_global_param() {
        let event_id = EventId::new();
        let surface_id = ParameterId::new();
        let switch_id = NodeId::new();
        let wood_asset = Uuid::now_v7();
        let stone_asset = Uuid::now_v7();
        let (wood_node_id, wood_sampler) = make_sampler(wood_asset);
        let (stone_node_id, stone_sampler) = make_sampler(stone_asset);

        let event = make_event(
            event_id,
            switch_id,
            vec![
                EventContentNode::Switch(SwitchNode {
                    id: switch_id,
                    parameter_id: surface_id,
                    cases: vec![
                        SwitchCase {
                            variant: "wood".into(),
                            child: NodeRef { id: wood_node_id },
                        },
                        SwitchCase {
                            variant: "stone".into(),
                            child: NodeRef { id: stone_node_id },
                        },
                    ],
                    default_case: Some(NodeRef { id: wood_node_id }),
                }),
                wood_sampler,
                stone_sampler,
            ],
        );

        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");
        runtime
            .set_global_param(surface_id, ParameterValue::Enum("stone".into()))
            .expect("param should set");

        let plan = runtime.plan_event(event_id).expect("plan should resolve");

        assert_eq!(plan.asset_ids, vec![stone_asset]);
    }

    #[test]
    fn plan_event_on_prefers_emitter_param_over_global_param() {
        let event_id = EventId::new();
        let surface_id = ParameterId::new();
        let switch_id = NodeId::new();
        let wood_asset = Uuid::now_v7();
        let stone_asset = Uuid::now_v7();
        let (wood_node_id, wood_sampler) = make_sampler(wood_asset);
        let (stone_node_id, stone_sampler) = make_sampler(stone_asset);

        let event = make_event(
            event_id,
            switch_id,
            vec![
                EventContentNode::Switch(SwitchNode {
                    id: switch_id,
                    parameter_id: surface_id,
                    cases: vec![
                        SwitchCase {
                            variant: "wood".into(),
                            child: NodeRef { id: wood_node_id },
                        },
                        SwitchCase {
                            variant: "stone".into(),
                            child: NodeRef { id: stone_node_id },
                        },
                    ],
                    default_case: Some(NodeRef { id: wood_node_id }),
                }),
                wood_sampler,
                stone_sampler,
            ],
        );

        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");
        let emitter_id = runtime.create_emitter();
        runtime
            .set_global_param(surface_id, ParameterValue::Enum("wood".into()))
            .expect("param should set");
        runtime
            .set_emitter_param(emitter_id, surface_id, ParameterValue::Enum("stone".into()))
            .expect("emitter param should set");

        let plan = runtime
            .plan_event_on(emitter_id, event_id)
            .expect("plan should resolve");

        assert_eq!(plan.asset_ids, vec![stone_asset]);
        assert_eq!(plan.emitter_id, Some(emitter_id));
    }

    #[test]
    fn plan_event_resolves_sequence_children_in_order() {
        let event_id = EventId::new();
        let root_id = NodeId::new();
        let asset_a = Uuid::now_v7();
        let asset_b = Uuid::now_v7();
        let (node_a, sampler_a) = make_sampler(asset_a);
        let (node_b, sampler_b) = make_sampler(asset_b);

        let event = make_event(
            event_id,
            root_id,
            vec![
                EventContentNode::Sequence(SequenceNode {
                    id: root_id,
                    children: vec![NodeRef { id: node_a }, NodeRef { id: node_b }],
                }),
                sampler_a,
                sampler_b,
            ],
        );

        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let plan = runtime.plan_event(event_id).expect("plan should resolve");

        assert_eq!(plan.asset_ids, vec![asset_a, asset_b]);
    }

    #[test]
    fn audio_command_buffer_applies_requests_in_order() {
        let mut buffer = AudioCommandBuffer::new();
        buffer.push(1);
        buffer.push(2);

        let results = buffer
            .apply(|value| Ok::<_, ()>(value * 10))
            .expect("apply should succeed");

        assert_eq!(results, vec![10, 20]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn audio_command_buffer_isolates_per_request_failures() {
        let mut buffer = AudioCommandBuffer::new();
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        let outcomes = buffer.apply_isolated(|value| {
            if *value == 2 {
                Err("boom")
            } else {
                Ok(value * 10)
            }
        });

        assert_eq!(outcomes.len(), 3);
        assert!(matches!(outcomes[0].result, Ok(10)));
        assert!(matches!(outcomes[1].result, Err("boom")));
        assert!(matches!(outcomes[2].result, Ok(30)));
        assert!(buffer.is_empty());
    }

    #[test]
    fn stop_request_removes_active_instance() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let (sampler_id, sampler) = make_sampler(asset_id);
        let event = make_event(event_id, sampler_id, vec![sampler]);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let instance_id = runtime.play(event_id).expect("event should play");
        let result = runtime
            .apply_request(&RuntimeRequest::stop(instance_id, Fade::IMMEDIATE))
            .expect("stop should succeed");

        assert_eq!(result, RuntimeRequestResult::Stopped { instance_id });
        assert_eq!(runtime.active_plan(instance_id), None);
    }

    #[test]
    fn instance_state_reports_playing_then_stopped() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let (sampler_id, sampler) = make_sampler(asset_id);
        let event = make_event(event_id, sampler_id, vec![sampler]);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let instance_id = runtime.play(event_id).expect("event should play");
        assert_eq!(
            runtime.instance_state(instance_id),
            EventInstanceState::Playing
        );

        runtime
            .stop(instance_id, Fade::IMMEDIATE)
            .expect("stop should succeed");
        assert_eq!(
            runtime.instance_state(instance_id),
            EventInstanceState::Stopped
        );
    }

    #[test]
    fn load_bank_keeps_only_compiled_objects_in_runtime_state() {
        let event_id = EventId::new();
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);
        let bank_id = bank.id;

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, Vec::new())
            .expect("bank should load");

        let objects = runtime
            .loaded_bank_objects(bank_id)
            .expect("loaded bank objects should exist");

        assert_eq!(objects.events, vec![event_id]);
    }

    #[test]
    fn load_bank_with_definitions_registers_music_foundation_objects() {
        let asset_id = Uuid::now_v7();
        let clip = Clip::new("explore_main", asset_id);
        let resume_slot = ResumeSlot::new("explore_memory");
        let sync_domain = SyncDomain::new("day_night");
        let state_id = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("world_music");
        graph.initial_node = Some(state_id);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: state_id,
            name: "explore".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            }],
            memory_slot: Some(resume_slot.id),
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.push(clip.id);
        bank.objects.resume_slots.push(resume_slot.id);
        bank.objects.sync_domains.push(sync_domain.id);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![clip.clone()],
                vec![resume_slot.clone()],
                vec![sync_domain.clone()],
                vec![graph.clone()],
            )
            .expect("bank should load with music definitions");

        assert_eq!(runtime.clip(clip.id), Some(&clip));
        assert_eq!(runtime.resume_slot(resume_slot.id), Some(&resume_slot));
        assert_eq!(runtime.sync_domain(sync_domain.id), Some(&sync_domain));
        assert_eq!(runtime.music_graph(graph.id), Some(&graph));
    }

    #[test]
    fn unload_bank_removes_music_foundation_objects() {
        let asset_id = Uuid::now_v7();
        let clip = Clip::new("combat_main", asset_id);
        let resume_slot = ResumeSlot::new("combat_memory");
        let state_id = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("combat_music");
        graph.initial_node = Some(state_id);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: state_id,
            name: "combat".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            }],
            memory_slot: Some(resume_slot.id),
            memory_policy: MemoryPolicy {
                ttl_seconds: Some(30.0),
                reset_to: EntryPolicy::ClipStart,
            },
            default_entry: EntryPolicy::Resume,
            externally_targetable: true,
            completion_source: None,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.push(clip.id);
        bank.objects.resume_slots.push(resume_slot.id);
        bank.objects.music_graphs.push(graph.id);
        let bank_id = bank.id;

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![clip.clone()],
                vec![resume_slot.clone()],
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with music definitions");

        runtime.unload_bank(bank_id).expect("bank should unload");

        assert_eq!(runtime.clip(clip.id), None);
        assert_eq!(runtime.resume_slot(resume_slot.id), None);
        assert_eq!(runtime.music_graph(graph.id), None);
    }

    #[test]
    fn play_music_graph_uses_declared_initial_state() {
        let asset_id = Uuid::now_v7();
        let clip = Clip::new("explore_main", asset_id);
        let explore_state = MusicStateId::new();
        let combat_state = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("world_music");
        graph.initial_node = Some(combat_state);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: explore_state,
            name: "explore".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });
        graph.nodes.push(MusicStateNode {
            id: combat_state,
            name: "combat".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.push(clip.id);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![clip],
                Vec::new(),
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with music graph");

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        let status = runtime
            .music_status(session_id)
            .expect("music status should resolve");

        assert_eq!(status.active_node, combat_state);
        assert_eq!(status.desired_target_node, combat_state);
        assert_eq!(status.phase, MusicPhase::Stable);
    }

    #[test]
    fn request_music_state_tracks_pending_transition_until_bridge_completes() {
        let asset_id = Uuid::now_v7();
        let clip = Clip::new("preheat_loop", asset_id);
        let bridge_clip = Clip::new("transition", Uuid::now_v7());
        let boss_clip = Clip::new("boss_loop", Uuid::now_v7());
        let preheat_state = MusicStateId::new();
        let bridge_state = MusicStateId::new();
        let boss_state = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let bridge_track = Track::new("music_bridge", TrackRole::Bridge);
        let mut graph = MusicGraph::new("boss_music");
        graph.initial_node = Some(preheat_state);
        graph.tracks.push(main_track.clone());
        graph.tracks.push(bridge_track.clone());
        graph.nodes.push(MusicStateNode {
            id: preheat_state,
            name: "preheat".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });
        graph.nodes.push(MusicStateNode {
            id: bridge_state,
            name: "bridge".into(),
            bindings: vec![TrackBinding {
                track_id: bridge_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: bridge_clip.id,
                },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: false,
            completion_source: Some(bridge_track.id),
        });
        graph.nodes.push(MusicStateNode {
            id: boss_state,
            name: "boss".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: boss_clip.id,
                },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });
        graph.edges.push(TransitionRule {
            from: preheat_state,
            to: bridge_state,
            requested_target: Some(boss_state),
            trigger: ExitPolicy::NextMatchingCue {
                tag: "battle_ready".into(),
            },
            destination: EntryPolicy::ClipStart,
        });
        graph.edges.push(TransitionRule {
            from: bridge_state,
            to: boss_state,
            requested_target: Some(boss_state),
            trigger: ExitPolicy::OnComplete,
            destination: EntryPolicy::EntryCue {
                tag: "boss_in".into(),
            },
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.push(clip.id);
        bank.objects.clips.push(bridge_clip.id);
        bank.objects.clips.push(boss_clip.id);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![clip.clone(), bridge_clip.clone(), boss_clip.clone()],
                Vec::new(),
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with music graph");

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        runtime
            .request_music_state(session_id, boss_state)
            .expect("state request should succeed");

        let waiting_status = runtime
            .music_status(session_id)
            .expect("music status should resolve");
        assert_eq!(waiting_status.active_node, preheat_state);
        assert_eq!(waiting_status.desired_target_node, boss_state);
        assert_eq!(waiting_status.phase, MusicPhase::WaitingExitCue);
        assert_eq!(waiting_status.current_track_id, Some(main_track.id));

        runtime
            .complete_music_exit(session_id)
            .expect("exit cue completion should succeed");
        let bridge_status = runtime
            .music_status(session_id)
            .expect("music status should resolve");
        assert_eq!(bridge_status.phase, MusicPhase::WaitingNodeCompletion);
        assert_eq!(bridge_status.current_track_id, Some(bridge_track.id));

        let bridge_playback = runtime
            .resolve_music_playback(session_id, 0.0)
            .expect("bridge playback should resolve");
        assert_eq!(bridge_playback.clip_id, bridge_clip.id);
        assert_eq!(bridge_playback.track_id, Some(bridge_track.id));

        runtime
            .complete_music_node_completion(session_id)
            .expect("bridge completion should succeed");
        let stable_status = runtime
            .music_status(session_id)
            .expect("music status should resolve");
        assert_eq!(stable_status.active_node, boss_state);
        assert_eq!(stable_status.desired_target_node, boss_state);
        assert_eq!(stable_status.phase, MusicPhase::Stable);
        assert!(stable_status.pending_transition.is_none());
    }

    #[test]
    fn resolve_music_playback_uses_saved_resume_position_when_memory_is_fresh() {
        let clip = Clip::new("explore_main", Uuid::now_v7());
        let resume_slot = ResumeSlot::new("explore_memory");
        let state_id = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("world_music");
        graph.initial_node = Some(state_id);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: state_id,
            name: "explore".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            }],
            memory_slot: Some(resume_slot.id),
            memory_policy: MemoryPolicy {
                ttl_seconds: Some(30.0),
                reset_to: EntryPolicy::ClipStart,
            },
            default_entry: EntryPolicy::Resume,
            externally_targetable: true,
            completion_source: None,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.push(clip.id);
        bank.objects.resume_slots.push(resume_slot.id);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![clip.clone()],
                vec![resume_slot.clone()],
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with music graph");

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        assert!(
            runtime
                .save_music_session_resume_position(session_id, 12.5, 10.0)
                .expect("resume save should succeed")
        );

        let resolved = runtime
            .resolve_music_playback(session_id, 20.0)
            .expect("music playback should resolve");

        assert_eq!(resolved.clip_id, clip.id);
        assert_eq!(resolved.entry_offset_seconds, 12.5);
        assert_eq!(
            runtime
                .resume_memory(resume_slot.id)
                .unwrap()
                .position_seconds,
            12.5
        );
    }

    #[test]
    fn resolve_music_playback_falls_back_to_clip_start_after_resume_ttl_expires() {
        let clip = Clip::new("explore_main", Uuid::now_v7());
        let resume_slot = ResumeSlot::new("explore_memory");
        let state_id = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("world_music");
        graph.initial_node = Some(state_id);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: state_id,
            name: "explore".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            }],
            memory_slot: Some(resume_slot.id),
            memory_policy: MemoryPolicy {
                ttl_seconds: Some(5.0),
                reset_to: EntryPolicy::ClipStart,
            },
            default_entry: EntryPolicy::Resume,
            externally_targetable: true,
            completion_source: None,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.push(clip.id);
        bank.objects.resume_slots.push(resume_slot.id);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![clip],
                vec![resume_slot],
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with music graph");

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        runtime
            .save_music_session_resume_position(session_id, 9.0, 10.0)
            .expect("resume save should succeed");

        let resolved = runtime
            .resolve_music_playback(session_id, 20.0)
            .expect("music playback should resolve");

        assert_eq!(resolved.entry_offset_seconds, 0.0);
    }

    #[test]
    fn immediate_music_transition_uses_destination_resume_entry() {
        let explore_clip = Clip::new("explore_main", Uuid::now_v7());
        let combat_clip = Clip::new("combat_main", Uuid::now_v7());
        let combat_memory = ResumeSlot::new("combat_memory");
        let explore_state = MusicStateId::new();
        let combat_state = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("world_music");
        graph.initial_node = Some(explore_state);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: explore_state,
            name: "explore".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: explore_clip.id,
                },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });
        graph.nodes.push(MusicStateNode {
            id: combat_state,
            name: "combat".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: combat_clip.id,
                },
            }],
            memory_slot: Some(combat_memory.id),
            memory_policy: MemoryPolicy {
                ttl_seconds: Some(30.0),
                reset_to: EntryPolicy::ClipStart,
            },
            default_entry: EntryPolicy::Resume,
            externally_targetable: true,
            completion_source: None,
        });
        graph.edges.push(TransitionRule {
            from: explore_state,
            to: combat_state,
            requested_target: None,
            trigger: ExitPolicy::Immediate,
            destination: EntryPolicy::Resume,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.extend([explore_clip.id, combat_clip.id]);
        bank.objects.resume_slots.push(combat_memory.id);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![explore_clip, combat_clip.clone()],
                vec![combat_memory.clone()],
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with music graph");

        runtime.resume_memories.insert(
            combat_memory.id,
            ResumeMemoryEntry {
                position_seconds: 18.0,
                saved_at_seconds: 10.0,
            },
        );

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        runtime
            .request_music_state(session_id, combat_state)
            .expect("music state request should succeed");

        let resolved = runtime
            .resolve_music_playback(session_id, 20.0)
            .expect("music playback should resolve");

        assert_eq!(resolved.clip_id, combat_clip.id);
        assert_eq!(resolved.entry_offset_seconds, 18.0);
    }

    #[test]
    fn resolve_music_playback_uses_entry_cue_offset() {
        let mut clip = Clip::new("boss_loop", Uuid::now_v7());
        let mut first = CuePoint::new("boss_intro", 4.0);
        first.tags.push("boss_in".into());
        let mut second = CuePoint::new("boss_intro_2", 9.0);
        second.tags.push("boss_in".into());
        clip.cues = vec![second, first];

        let state_id = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("boss_music");
        graph.initial_node = Some(state_id);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: state_id,
            name: "boss".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip { clip_id: clip.id },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::EntryCue {
                tag: "boss_in".into(),
            },
            externally_targetable: true,
            completion_source: None,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.push(clip.id);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![clip.clone()],
                Vec::new(),
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with music graph");

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        let resolved = runtime
            .resolve_music_playback(session_id, 0.0)
            .expect("music playback should resolve");

        assert_eq!(resolved.clip_id, clip.id);
        assert_eq!(resolved.entry_offset_seconds, 4.0);
    }

    #[test]
    fn explicit_main_track_binding_drives_playback_target() {
        let main_clip = Clip::new("explicit_main", Uuid::now_v7());
        let state_id = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("boss_music");
        graph.initial_node = Some(state_id);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: state_id,
            name: "boss".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: main_clip.id,
                },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.push(main_clip.id);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![main_clip.clone()],
                Vec::new(),
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with explicit main track");

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        let status = runtime
            .music_status(session_id)
            .expect("music status should resolve");
        let resolved = runtime
            .resolve_music_playback(session_id, 0.0)
            .expect("music playback should resolve");

        assert_eq!(
            status.current_target,
            PlaybackTarget::Clip {
                clip_id: main_clip.id,
            }
        );
        assert_eq!(resolved.clip_id, main_clip.id);
    }

    #[test]
    fn resolve_music_stinger_playback_uses_active_bridge_node_stinger_track() {
        let preheat_clip = Clip::new("preheat_loop", Uuid::now_v7());
        let bridge_clip = Clip::new("boss_bridge", Uuid::now_v7());
        let boss_clip = Clip::new("boss_loop", Uuid::now_v7());
        let stinger_clip = Clip::new("boss_hit", Uuid::now_v7());
        let preheat_state = MusicStateId::new();
        let bridge_state = MusicStateId::new();
        let boss_state = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let bridge_track = Track::new("music_bridge", TrackRole::Bridge);
        let stinger_track = Track::new("music_stinger", TrackRole::Stinger);
        let mut graph = MusicGraph::new("boss_music");
        graph.initial_node = Some(preheat_state);
        graph.tracks.push(main_track.clone());
        graph.tracks.push(bridge_track.clone());
        graph.tracks.push(stinger_track.clone());
        graph.nodes.push(MusicStateNode {
            id: preheat_state,
            name: "preheat".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: preheat_clip.id,
                },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });
        graph.nodes.push(MusicStateNode {
            id: bridge_state,
            name: "bridge".into(),
            bindings: vec![
                TrackBinding {
                    track_id: bridge_track.id,
                    target: PlaybackTarget::Clip {
                        clip_id: bridge_clip.id,
                    },
                },
                TrackBinding {
                    track_id: stinger_track.id,
                    target: PlaybackTarget::Clip {
                        clip_id: stinger_clip.id,
                    },
                },
            ],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: false,
            completion_source: Some(bridge_track.id),
        });
        graph.nodes.push(MusicStateNode {
            id: boss_state,
            name: "boss".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: boss_clip.id,
                },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });
        graph.edges.push(TransitionRule {
            from: preheat_state,
            to: bridge_state,
            requested_target: Some(boss_state),
            trigger: ExitPolicy::NextMatchingCue {
                tag: "battle_ready".into(),
            },
            destination: EntryPolicy::ClipStart,
        });
        graph.edges.push(TransitionRule {
            from: bridge_state,
            to: boss_state,
            requested_target: Some(boss_state),
            trigger: ExitPolicy::OnComplete,
            destination: EntryPolicy::ClipStart,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.extend([
            preheat_clip.id,
            bridge_clip.id,
            boss_clip.id,
            stinger_clip.id,
        ]);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![preheat_clip, bridge_clip, boss_clip, stinger_clip.clone()],
                Vec::new(),
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with stinger track");

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        runtime
            .request_music_state(session_id, boss_state)
            .expect("state request should succeed");
        runtime
            .complete_music_exit(session_id)
            .expect("exit cue completion should succeed");

        let stinger = runtime
            .resolve_music_stinger_playback(session_id)
            .expect("stinger playback should resolve")
            .expect("bridge node should expose stinger playback");
        assert_eq!(stinger.clip_id, stinger_clip.id);
        assert_eq!(stinger.track_id, Some(stinger_track.id));
    }

    #[test]
    fn find_next_music_exit_cue_prefers_current_cycle_then_wraps_looping_clip() {
        let mut preheat_clip = Clip::new("preheat_loop", Uuid::now_v7());
        preheat_clip.loop_range = Some(TimeRange::new(0.0, 12.0));
        let mut cue_a = CuePoint::new("bar_1", 2.0);
        cue_a.tags.push("battle_ready".into());
        let mut cue_b = CuePoint::new("bar_2", 8.0);
        cue_b.tags.push("battle_ready".into());
        preheat_clip.cues = vec![cue_a, cue_b];

        let boss_clip = Clip::new("boss_loop", Uuid::now_v7());
        let preheat_state = MusicStateId::new();
        let boss_state = MusicStateId::new();
        let main_track = Track::new("music_main", TrackRole::Main);
        let mut graph = MusicGraph::new("boss_music");
        graph.initial_node = Some(preheat_state);
        graph.tracks.push(main_track.clone());
        graph.nodes.push(MusicStateNode {
            id: preheat_state,
            name: "preheat".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: preheat_clip.id,
                },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });
        graph.nodes.push(MusicStateNode {
            id: boss_state,
            name: "boss".into(),
            bindings: vec![TrackBinding {
                track_id: main_track.id,
                target: PlaybackTarget::Clip {
                    clip_id: boss_clip.id,
                },
            }],
            memory_slot: None,
            memory_policy: MemoryPolicy::default(),
            default_entry: EntryPolicy::ClipStart,
            externally_targetable: true,
            completion_source: None,
        });
        graph.edges.push(TransitionRule {
            from: preheat_state,
            to: boss_state,
            requested_target: None,
            trigger: ExitPolicy::NextMatchingCue {
                tag: "battle_ready".into(),
            },
            destination: EntryPolicy::ClipStart,
        });

        let mut bank = Bank::new("core");
        bank.objects.clips.extend([preheat_clip.id, boss_clip.id]);
        bank.objects.music_graphs.push(graph.id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank_with_definitions(
                bank,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![preheat_clip, boss_clip],
                Vec::new(),
                Vec::new(),
                vec![graph.clone()],
            )
            .expect("bank should load with music graph");

        let session_id = runtime
            .play_music_graph(graph.id)
            .expect("music graph should start");
        runtime
            .request_music_state(session_id, boss_state)
            .expect("state request should succeed");

        let current_cycle = runtime
            .find_next_music_exit_cue(session_id, 3.0)
            .expect("cue lookup should succeed")
            .expect("matching cue should exist");
        assert_eq!(current_cycle.cue_position_seconds, 8.0);
        assert!(!current_cycle.requires_wrap);

        let next_cycle = runtime
            .find_next_music_exit_cue(session_id, 9.0)
            .expect("cue lookup should succeed")
            .expect("matching cue should exist");
        assert_eq!(next_cycle.cue_position_seconds, 2.0);
        assert!(next_cycle.requires_wrap);
    }

    #[test]
    fn queued_runtime_applies_buffered_requests_against_runtime_state() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let (sampler_id, sampler) = make_sampler(asset_id);
        let event = make_event(event_id, sampler_id, vec![sampler]);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut queued = QueuedRuntime::new();
        queued
            .load_bank(bank, vec![event])
            .expect("bank should load");
        queued.queue_play(event_id);

        let results = queued
            .apply_requests()
            .expect("queued requests should apply");

        let instance_id = match results.last() {
            Some(RuntimeRequestResult::Played { instance_id }) => *instance_id,
            other => panic!("expected final played result, got {other:?}"),
        };

        assert_eq!(
            queued.active_plan(instance_id).map(|plan| &plan.asset_ids),
            Some(&vec![asset_id])
        );
    }

    #[test]
    fn push_snapshot_creates_active_instance_and_updates_bus_volume() {
        let bus_id = BusId::new();
        let snapshot = Snapshot {
            id: SnapshotId::new(),
            name: "combat".into(),
            fade_in_seconds: 0.2,
            fade_out_seconds: 0.4,
            targets: vec![SnapshotTarget {
                bus_id,
                target_volume: 0.65,
            }],
        };
        let mut bank = Bank::new("core");
        bank.objects.buses.push(bus_id);

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, Vec::new())
            .expect("bank should load");
        runtime.load_snapshot(snapshot.clone());

        let instance_id = runtime
            .push_snapshot(snapshot.id, Fade::seconds(snapshot.fade_in_seconds))
            .expect("snapshot should push");

        assert_eq!(runtime.bus_volume(bus_id), Some(0.65));
        let active = runtime
            .active_snapshot(instance_id)
            .expect("active snapshot should exist");
        assert_eq!(active.snapshot_id, snapshot.id);
        assert_eq!(active.overrides.get(&bus_id), Some(&0.65));
    }

    #[test]
    fn push_snapshot_rejects_unknown_target_bus() {
        let snapshot = Snapshot {
            id: SnapshotId::new(),
            name: "combat".into(),
            fade_in_seconds: 0.2,
            fade_out_seconds: 0.4,
            targets: vec![SnapshotTarget {
                bus_id: BusId::new(),
                target_volume: 0.65,
            }],
        };

        let mut runtime = SonaraRuntime::new();
        runtime.load_snapshot(snapshot.clone());

        assert!(matches!(
            runtime.push_snapshot(snapshot.id, Fade::IMMEDIATE),
            Err(RuntimeError::SnapshotTargetBusNotFound(_))
        ));
    }

    #[test]
    fn active_bus_volume_follows_event_default_bus_override() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let bus_id = BusId::new();
        let (sampler_id, sampler) = make_sampler(asset_id);
        let mut event = make_event(event_id, sampler_id, vec![sampler]);
        event.default_bus = Some(bus_id);

        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);
        bank.objects.buses.push(bus_id);

        let snapshot = Snapshot {
            id: SnapshotId::new(),
            name: "combat".into(),
            fade_in_seconds: 0.2,
            fade_out_seconds: 0.4,
            targets: vec![SnapshotTarget {
                bus_id,
                target_volume: 0.4,
            }],
        };

        let mut runtime = SonaraRuntime::new();
        runtime
            .load_bank(bank, vec![event.clone()])
            .expect("bank should load");
        runtime.load_snapshot(snapshot.clone());
        runtime
            .push_snapshot(snapshot.id, Fade::IMMEDIATE)
            .expect("snapshot should push");

        let instance_id = runtime.play(event.id).expect("event should play");

        assert_eq!(runtime.active_bus_volume(instance_id), Some(0.4));
    }
}
