// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};

use crate::ids::BusEffectSlotId;

pub const LOW_PASS_MIN_CUTOFF_HZ: f32 = 20.0;
pub const LOW_PASS_MAX_CUTOFF_HZ: f32 = 20_480.0;

/// 挂在 bus 上的一个 effect slot。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusEffectSlot {
    pub id: BusEffectSlotId,
    pub effect: BusEffect,
}

impl BusEffectSlot {
    /// 创建一个新的 low-pass slot。
    pub fn low_pass(cutoff_hz: f32) -> Self {
        Self {
            id: BusEffectSlotId::new(),
            effect: BusEffect::LowPass(LowPassEffect::new(cutoff_hz)),
        }
    }

    /// 如果这个 slot 是 low-pass，则返回其只读参数。
    pub fn low_pass_effect(&self) -> Option<&LowPassEffect> {
        match &self.effect {
            BusEffect::LowPass(effect) => Some(effect),
        }
    }

    /// 如果这个 slot 是 low-pass，则返回其可变参数。
    pub fn low_pass_effect_mut(&mut self) -> Option<&mut LowPassEffect> {
        match &mut self.effect {
            BusEffect::LowPass(effect) => Some(effect),
        }
    }
}

/// bus effect 的当前最小集合。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BusEffect {
    LowPass(LowPassEffect),
}

/// 一个简单的 low-pass 效果。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LowPassEffect {
    pub enabled: bool,
    pub cutoff_hz: f32,
}

impl Default for LowPassEffect {
    fn default() -> Self {
        Self::new(1_000.0)
    }
}

impl LowPassEffect {
    /// 创建一个新的 low-pass effect。
    pub fn new(cutoff_hz: f32) -> Self {
        Self {
            enabled: true,
            cutoff_hz: clamp_low_pass_cutoff_hz(cutoff_hz),
        }
    }

    /// 更新 cutoff，并自动夹紧到 Firewheel fast low-pass 的有效范围。
    pub fn set_cutoff_hz(&mut self, cutoff_hz: f32) {
        self.cutoff_hz = clamp_low_pass_cutoff_hz(cutoff_hz);
    }
}

pub fn clamp_low_pass_cutoff_hz(cutoff_hz: f32) -> f32 {
    cutoff_hz.clamp(LOW_PASS_MIN_CUTOFF_HZ, LOW_PASS_MAX_CUTOFF_HZ)
}
