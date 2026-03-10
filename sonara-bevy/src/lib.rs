//! Bevy 集成层骨架

use sonara_model::{Bank, BankId, Event, EventId, ParameterId, ParameterValue, SnapshotId};
use sonara_runtime::{
    EmitterId, EventInstanceId, Fade, RuntimeError, SnapshotInstanceId, SonaraRuntime,
};

/// Sonara 的 Bevy 插件入口
#[derive(Debug, Default)]
pub struct SonaraPlugin;

/// Bevy 侧的全局音频入口
#[derive(Debug, Default)]
pub struct SonaraAudio {
    runtime: SonaraRuntime,
}

impl SonaraAudio {
    /// 创建一个新的音频入口
    pub fn new() -> Self {
        Self {
            runtime: SonaraRuntime::new(),
        }
    }

    /// 加载一个 bank
    ///
    /// 这里先保留最小骨架, 后续会改成真正的加载流程
    pub fn load_bank(&mut self, bank: Bank, events: Vec<Event>) -> Result<BankId, RuntimeError> {
        self.runtime.load_bank(bank, events)
    }

    /// 播放一个未绑定实体的事件
    pub fn play(&mut self, event_id: EventId) -> Result<EventInstanceId, RuntimeError> {
        self.runtime.play(event_id)
    }

    /// 创建一个 emitter
    pub fn create_emitter(&mut self) -> EmitterId {
        self.runtime.create_emitter()
    }

    /// 在 emitter 上播放一个事件
    pub fn play_on(
        &mut self,
        emitter_id: EmitterId,
        event_id: EventId,
    ) -> Result<EventInstanceId, RuntimeError> {
        self.runtime.play_on(emitter_id, event_id)
    }

    /// 设置一个全局参数
    pub fn set_global_param(
        &mut self,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        self.runtime.set_global_param(parameter_id, value)
    }

    /// 设置 emitter 参数
    pub fn set_emitter_param(
        &mut self,
        emitter_id: EmitterId,
        parameter_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), RuntimeError> {
        self.runtime
            .set_emitter_param(emitter_id, parameter_id, value)
    }

    /// 停止一个事件实例
    pub fn stop(&mut self, instance_id: EventInstanceId, fade: Fade) -> Result<(), RuntimeError> {
        self.runtime.stop(instance_id, fade)
    }

    /// 压入一个 snapshot
    pub fn push_snapshot(
        &mut self,
        snapshot_id: SnapshotId,
        fade: Fade,
    ) -> Result<SnapshotInstanceId, RuntimeError> {
        self.runtime.push_snapshot(snapshot_id, fade)
    }
}

/// 绑定到实体上的发声体组件
#[derive(Debug, Default)]
pub struct AudioEmitter {
    pub enabled: bool,
}

/// 绑定到实体上的监听器组件
#[derive(Debug, Default)]
pub struct AudioListener {
    pub enabled: bool,
}
