// SPDX-License-Identifier: MPL-2.0

use bevy_ecs::prelude::Component;
use sonara_runtime::EmitterId;

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
