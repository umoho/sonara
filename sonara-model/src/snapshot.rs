// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::ids::{BusId, SnapshotId};

/// 一组针对 bus 的混音覆盖
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: SnapshotId,
    pub name: SmolStr,
    pub fade_in_seconds: f32,
    pub fade_out_seconds: f32,
    pub targets: Vec<SnapshotTarget>,
}

/// Snapshot 对某个 bus 的目标覆盖
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotTarget {
    pub bus_id: BusId,
    pub target_volume: f32,
}
