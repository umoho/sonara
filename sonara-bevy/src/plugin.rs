// SPDX-License-Identifier: MPL-2.0

use bevy_app::{App, Plugin, Update};

use crate::audio::{SonaraAudio, update_firewheel_backend_system};

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
