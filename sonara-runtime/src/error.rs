// SPDX-License-Identifier: MPL-2.0

use sonara_model::{
    BankId, BusEffectSlotId, BusId, EventId, MusicGraphId, MusicNodeId, NodeId, ParameterId,
    SnapshotId, TrackGroupId,
};
use thiserror::Error;

use crate::ids::{EmitterId, EventInstanceId, MusicSessionId};
use crate::types::MusicPhase;

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
    #[error("bus `{0:?}` 不存在")]
    BusNotLoaded(BusId),
    #[error("bus `{bus_id:?}` 上不存在 effect slot `{slot_id:?}`")]
    BusEffectSlotNotFound {
        bus_id: BusId,
        slot_id: BusEffectSlotId,
    },
    #[error("music graph `{0:?}` is not loaded")]
    MusicGraphNotLoaded(MusicGraphId),
    #[error("music graph `{0:?}` has no nodes")]
    MusicGraphHasNoNodes(MusicGraphId),
    #[error("music graph `{graph_id:?}` has no node `{node_id:?}`")]
    MusicNodeNotFound {
        graph_id: MusicGraphId,
        node_id: MusicNodeId,
    },
    #[error("music graph `{graph_id:?}` has no active playable track on node `{node_id:?}`")]
    MusicNodeHasNoActiveTrack {
        graph_id: MusicGraphId,
        node_id: MusicNodeId,
    },
    #[error("music session `{0:?}` 不存在")]
    MusicSessionNotFound(MusicSessionId),
    #[error("music graph `{graph_id:?}` has no edge `{from:?} -> {to:?}`")]
    MusicEdgeNotFound {
        graph_id: MusicGraphId,
        from: MusicNodeId,
        to: MusicNodeId,
    },
    #[error("music graph `{graph_id:?}` has no track group `{group_id:?}`")]
    MusicTrackGroupNotFound {
        graph_id: MusicGraphId,
        group_id: TrackGroupId,
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
