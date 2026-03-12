//! Bevy 集成层骨架

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::{Component, NonSendMut};
use sonara_build::CompiledBankPackage;
use sonara_firewheel::{FirewheelBackend, FirewheelBackendError};
use sonara_model::{
    Bank, BankId, Bus, Clip, Event, EventId, MusicGraph, ParameterId, ParameterValue, ResumeSlot,
    Snapshot, SnapshotId, SyncDomain,
};
pub use sonara_runtime::EventInstanceState;
use sonara_runtime::{
    AudioCommandOutcome, EmitterId, EventInstanceId, Fade, PlaybackPlan, QueuedRuntime,
    RuntimeError, RuntimeRequest, RuntimeRequestResult, SnapshotInstanceId, SonaraRuntime,
};
use thiserror::Error;

/// Sonara 的默认 Bevy 插件入口。
///
/// 这个插件仍然使用纯 runtime 模式，适合快速测试 ECS 控制流。
#[derive(Debug, Default)]
pub struct SonaraPlugin;

impl Plugin for SonaraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(SonaraAudio::new());
    }
}

/// 带真实 Firewheel 后端的 Bevy 插件。
///
/// 这个插件会初始化音频输出，并在每帧自动推进后端。
#[derive(Debug, Default)]
pub struct SonaraFirewheelPlugin;

impl Plugin for SonaraFirewheelPlugin {
    fn build(&self, app: &mut App) {
        let audio = SonaraAudio::new_firewheel().expect("Firewheel backend should start");
        app.insert_non_send_resource(audio);
        app.add_systems(Update, update_firewheel_backend_system);
    }
}

/// Bevy 侧积累的一条音频请求
pub type AudioRequest = RuntimeRequest;

/// 一次请求执行后的结果
pub type AudioRequestResult = RuntimeRequestResult;

/// 一条请求在隔离执行模式下的结果
pub type AudioRequestOutcome =
    AudioCommandOutcome<AudioRequest, AudioRequestResult, AudioBackendError>;

/// Bevy 层统一暴露的 backend 错误。
#[derive(Debug, Error)]
pub enum AudioBackendError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Firewheel(#[from] FirewheelBackendError),
}

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

fn update_firewheel_backend_system(mut audio: NonSendMut<SonaraAudio>) {
    audio
        .update_backend()
        .expect("Firewheel backend update should succeed");
}

/// 绑定到实体上的发声体组件
#[derive(Debug, Default, Component)]
pub struct AudioEmitter {
    pub enabled: bool,
    pub id: Option<EmitterId>,
}

/// 绑定到实体上的监听器组件
#[derive(Debug, Default, Component)]
pub struct AudioListener {
    pub enabled: bool,
}

/// 便于 Bevy 游戏侧导入的最小预导出。
pub mod prelude {
    pub use crate::{
        AudioEmitter, AudioListener, AudioUpdate, EventInstanceState, SonaraAudio,
        SonaraFirewheelPlugin, SonaraPlugin,
    };
}

#[cfg(test)]
mod tests {
    use bevy_app::{App, Update};
    use bevy_ecs::{
        prelude::{Entity, NonSendMut},
        system::Single,
    };
    use sonara_model::{
        EventContentRoot, EventKind, NodeId, NodeRef, SamplerNode, SpatialMode, SwitchCase,
        SwitchNode,
    };
    use uuid::Uuid;

    use super::*;

    fn make_switch_event(event_id: EventId, parameter_id: ParameterId, asset_id: Uuid) -> Event {
        let switch_id = NodeId::new();
        let sampler_id = NodeId::new();

        Event {
            id: event_id,
            name: "player.footstep".into(),
            kind: EventKind::OneShot,
            root: EventContentRoot {
                root: NodeRef { id: switch_id },
                nodes: vec![
                    sonara_model::EventContentNode::Switch(SwitchNode {
                        id: switch_id,
                        parameter_id,
                        cases: vec![SwitchCase {
                            variant: "stone".into(),
                            child: NodeRef { id: sampler_id },
                        }],
                        default_case: Some(NodeRef { id: sampler_id }),
                    }),
                    sonara_model::EventContentNode::Sampler(SamplerNode {
                        id: sampler_id,
                        asset_id,
                    }),
                ],
            },
            default_bus: None,
            spatial: SpatialMode::ThreeD,
            default_parameters: Vec::new(),
            voice_limit: None,
            steal_policy: None,
        }
    }

    #[test]
    fn ensure_emitter_reuses_existing_id() {
        let mut audio = SonaraAudio::new();
        let mut emitter = AudioEmitter::default();

        let first = audio.ensure_emitter(&mut emitter);
        let second = audio.ensure_emitter(&mut emitter);

        assert_eq!(Some(first), emitter.id);
        assert_eq!(first, second);
    }

    #[test]
    fn detach_emitter_clears_bound_id() {
        let mut audio = SonaraAudio::new();
        let mut emitter = AudioEmitter::default();

        let _ = audio.ensure_emitter(&mut emitter);
        audio
            .detach_emitter(&mut emitter)
            .expect("detach should succeed");

        assert_eq!(None, emitter.id);
    }

    #[test]
    fn play_from_emitter_uses_component_bound_emitter() {
        let surface_id = ParameterId::new();
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let event = make_switch_event(event_id, surface_id, asset_id);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut audio = SonaraAudio::new();
        audio
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let mut emitter = AudioEmitter::default();
        audio
            .set_emitter_param_on(
                &mut emitter,
                surface_id,
                ParameterValue::Enum("stone".into()),
            )
            .expect("emitter param should set");

        let instance_id = audio
            .play_from_emitter(&mut emitter, event_id)
            .expect("play should succeed");
        let plan = audio.active_plan(instance_id).expect("plan should exist");

        assert_eq!(plan.emitter_id, emitter.id);
        assert_eq!(plan.asset_ids, vec![asset_id]);
    }

    #[test]
    fn queued_requests_are_applied_in_order() {
        let surface_id = ParameterId::new();
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let event = make_switch_event(event_id, surface_id, asset_id);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut audio = SonaraAudio::new();
        audio
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let mut emitter = AudioEmitter::default();
        audio.queue_set_emitter_param_on(
            &mut emitter,
            surface_id,
            ParameterValue::Enum("stone".into()),
        );
        audio.queue_play_from_emitter(&mut emitter, event_id);

        let results = audio.apply_requests().expect("requests should apply");
        let instance_id = match results.last() {
            Some(AudioRequestResult::Played { instance_id }) => *instance_id,
            other => panic!("expected final played result, got {other:?}"),
        };
        let plan = audio.active_plan(instance_id).expect("plan should exist");

        assert_eq!(results.len(), 2);
        assert_eq!(plan.emitter_id, emitter.id);
        assert_eq!(plan.asset_ids, vec![asset_id]);
    }

    #[test]
    fn isolated_request_application_keeps_processing_after_error() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let surface_id = ParameterId::new();
        let event = make_switch_event(event_id, surface_id, asset_id);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut audio = SonaraAudio::new();
        audio
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let missing_emitter = audio.create_emitter();
        let mut detached = AudioEmitter {
            enabled: true,
            id: Some(missing_emitter),
        };
        audio
            .detach_emitter(&mut detached)
            .expect("detach should succeed");

        audio.queue_play_on(missing_emitter, event_id);
        audio.queue_play(event_id);

        let outcomes = audio.apply_requests_isolated();

        assert_eq!(outcomes.len(), 2);
        assert!(matches!(
            outcomes[0].result,
            Err(AudioBackendError::Runtime(RuntimeError::EmitterNotFound(_)))
        ));
        assert!(matches!(
            outcomes[1].result,
            Ok(AudioRequestResult::Played { .. })
        ));
    }

    #[test]
    fn queued_stop_request_removes_active_instance() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let surface_id = ParameterId::new();
        let event = make_switch_event(event_id, surface_id, asset_id);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut audio = SonaraAudio::new();
        audio
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let instance_id = audio.play(event_id).expect("play should succeed");
        audio.queue_stop(instance_id, Fade::IMMEDIATE);

        let results = audio.apply_requests().expect("requests should apply");

        assert_eq!(results, vec![AudioRequestResult::Stopped { instance_id }]);
        assert_eq!(audio.active_plan(instance_id), None);
    }

    #[test]
    fn instance_state_reports_runtime_playback_lifecycle() {
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let surface_id = ParameterId::new();
        let event = make_switch_event(event_id, surface_id, asset_id);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut audio = SonaraAudio::new();
        audio
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let instance_id = audio.play(event_id).expect("play should succeed");
        assert_eq!(
            audio.instance_state(instance_id),
            EventInstanceState::Playing
        );

        audio
            .stop(instance_id, Fade::IMMEDIATE)
            .expect("stop should succeed");
        assert_eq!(
            audio.instance_state(instance_id),
            EventInstanceState::Stopped
        );
    }

    #[test]
    fn update_context_batches_emitter_commands_and_applies_them() {
        let surface_id = ParameterId::new();
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let event = make_switch_event(event_id, surface_id, asset_id);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut audio = SonaraAudio::new();
        audio
            .load_bank(bank, vec![event])
            .expect("bank should load");

        let mut emitter = AudioEmitter::default();
        let results = {
            let mut update = audio.begin_update();
            update.set_emitter_param_on(
                &mut emitter,
                surface_id,
                ParameterValue::Enum("stone".into()),
            );
            update.play_from_emitter(&mut emitter, event_id);
            update.apply().expect("update should apply")
        };

        let instance_id = match results.last() {
            Some(AudioRequestResult::Played { instance_id }) => *instance_id,
            other => panic!("expected final played result, got {other:?}"),
        };
        let plan = audio.active_plan(instance_id).expect("plan should exist");

        assert_eq!(results.len(), 2);
        assert_eq!(plan.emitter_id, emitter.id);
        assert_eq!(plan.asset_ids, vec![asset_id]);
    }

    #[test]
    fn plugin_exposes_audio_resource_to_real_bevy_update_system() {
        fn bevy_audio_system(
            mut audio: NonSendMut<SonaraAudio>,
            mut emitter: Single<&mut AudioEmitter>,
            event_id: NonSendMut<TestEventId>,
            surface_id: NonSendMut<TestSurfaceId>,
            mut played: NonSendMut<PlayedInstance>,
        ) {
            let mut update = audio.begin_update();
            update.set_emitter_param_on(
                &mut emitter,
                surface_id.0,
                ParameterValue::Enum("stone".into()),
            );
            update.play_from_emitter(&mut emitter, event_id.0);
            let results = update.apply().expect("update should apply");

            *played = PlayedInstance(match results.last() {
                Some(AudioRequestResult::Played { instance_id }) => Some(*instance_id),
                other => panic!("expected final played result, got {other:?}"),
            });
        }

        struct TestEventId(EventId);

        struct TestSurfaceId(ParameterId);

        #[derive(Default)]
        struct PlayedInstance(Option<EventInstanceId>);

        let surface_id = ParameterId::new();
        let event_id = EventId::new();
        let asset_id = Uuid::now_v7();
        let event = make_switch_event(event_id, surface_id, asset_id);
        let mut bank = Bank::new("core");
        bank.objects.events.push(event_id);

        let mut app = App::new();
        app.add_plugins(SonaraPlugin);
        app.insert_non_send_resource(TestEventId(event_id));
        app.insert_non_send_resource(TestSurfaceId(surface_id));
        app.insert_non_send_resource(PlayedInstance::default());
        app.world_mut().spawn(AudioEmitter::default());
        app.world_mut()
            .non_send_resource_mut::<SonaraAudio>()
            .load_bank(bank, vec![event])
            .expect("bank should load");
        app.add_systems(Update, bevy_audio_system);

        app.update();

        let played = app.world().non_send_resource::<PlayedInstance>().0;
        let instance_id = played.expect("system should have played an instance");
        let plan = app
            .world()
            .non_send_resource::<SonaraAudio>()
            .active_plan(instance_id)
            .expect("plan should exist");
        let plan_emitter_id = plan.emitter_id;
        let plan_asset_ids = plan.asset_ids.clone();
        let emitter_id = {
            let mut query = app.world_mut().query::<(Entity, &AudioEmitter)>();
            query
                .single(app.world())
                .expect("there should be one emitter entity")
                .1
                .id
        };

        assert_eq!(emitter_id, plan_emitter_id);
        assert_eq!(plan_asset_ids, vec![asset_id]);
    }
}
