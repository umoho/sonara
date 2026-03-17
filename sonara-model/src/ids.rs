// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        /// 稳定对象 ID
        ///
        /// 用于 bank 中的静态对象寻址和跨系统引用
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Uuid);

        impl $name {
            /// 生成一个新的稳定 ID
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

define_id!(BankId);
define_id!(BusEffectSlotId);
define_id!(BusId);
define_id!(ClipId);
define_id!(CueId);
define_id!(EventId);
define_id!(MusicGraphId);
define_id!(MusicNodeId);
define_id!(ParameterId);
define_id!(ResumeSlotId);
define_id!(SnapshotId);
define_id!(SyncDomainId);
define_id!(TrackId);
define_id!(TrackGroupId);
