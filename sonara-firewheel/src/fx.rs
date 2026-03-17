// SPDX-License-Identifier: MPL-2.0

use firewheel::{
    FirewheelCtx,
    backend::AudioBackend,
    channel_config::NonZeroChannelCount,
    clock::EventInstant,
    diff::{Diff, PathBuilder},
    node::NodeID,
    nodes::{
        fast_filters::lowpass::FastLowpassStereoNode,
        volume_pan::{VolumeNodeConfig, VolumePanNode},
    },
};
use firewheel_pool::{AudioNodePool, FxChain, SamplerPool};

pub(crate) const MAX_SUPPORTED_BUS_EFFECT_SLOTS: usize = 1;

pub(crate) type SamplerPoolSonaraFx = AudioNodePool<SamplerPool, SonaraFxChain>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SonaraFxChain {
    pub(crate) volume_pan: VolumePanNode,
    pub(crate) volume_pan_config: VolumeNodeConfig,
    pub(crate) low_pass: FastLowpassStereoNode,
}

impl Default for SonaraFxChain {
    fn default() -> Self {
        Self {
            volume_pan: VolumePanNode::default(),
            volume_pan_config: VolumeNodeConfig::default(),
            low_pass: FastLowpassStereoNode {
                enabled: false,
                ..Default::default()
            },
        }
    }
}

impl SonaraFxChain {
    pub(crate) fn set_volume_pan_params<B: AudioBackend>(
        &mut self,
        params: VolumePanNode,
        time: Option<EventInstant>,
        node_ids: &[NodeID],
        cx: &mut FirewheelCtx<B>,
    ) {
        let node_id = node_ids[0];
        self.volume_pan.diff(
            &params,
            PathBuilder::default(),
            &mut cx.event_queue_scheduled(node_id, time),
        );
    }

    pub(crate) fn set_low_pass_params<B: AudioBackend>(
        &mut self,
        params: FastLowpassStereoNode,
        time: Option<EventInstant>,
        node_ids: &[NodeID],
        cx: &mut FirewheelCtx<B>,
    ) {
        let node_id = node_ids[1];
        self.low_pass.diff(
            &params,
            PathBuilder::default(),
            &mut cx.event_queue_scheduled(node_id, time),
        );
    }
}

impl FxChain for SonaraFxChain {
    fn construct_and_connect<B: AudioBackend>(
        &mut self,
        first_node_id: NodeID,
        first_node_num_out_channels: NonZeroChannelCount,
        dst_node_id: NodeID,
        dst_num_channels: NonZeroChannelCount,
        cx: &mut FirewheelCtx<B>,
    ) -> Vec<NodeID> {
        let volume_pan_id = cx.add_node(self.volume_pan, Some(self.volume_pan_config));
        let low_pass_id = cx.add_node(self.low_pass, None);

        cx.connect(
            first_node_id,
            volume_pan_id,
            if first_node_num_out_channels.get().get() == 1 {
                &[(0, 0), (0, 1)]
            } else {
                &[(0, 0), (1, 1)]
            },
            false,
        )
        .unwrap();

        cx.connect(volume_pan_id, low_pass_id, &[(0, 0), (1, 1)], false)
            .unwrap();

        cx.connect(
            low_pass_id,
            dst_node_id,
            if dst_num_channels.get().get() == 1 {
                &[(0, 0), (1, 0)]
            } else {
                &[(0, 0), (1, 1)]
            },
            false,
        )
        .unwrap();

        vec![volume_pan_id, low_pass_id]
    }
}
