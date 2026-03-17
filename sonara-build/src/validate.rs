// SPDX-License-Identifier: MPL-2.0

use std::collections::{HashMap, HashSet};

use sonara_model::{
    Clip, ClipId, Event, EventContentNode, MusicGraph, NodeId, NodeRef, Parameter, ParameterId,
    ResumeSlot, ResumeSlotId, SyncDomain, SyncDomainId, TrackId,
};
use uuid::Uuid;

use crate::error::BuildError;

/// 对单个事件做最小语义校验
pub fn validate_event(event: &Event) -> Result<(), BuildError> {
    if event.root.nodes.is_empty() {
        return Err(BuildError::EmptyEventTree);
    }

    let mut node_ids = HashSet::new();
    let mut has_root = false;

    for node in &event.root.nodes {
        if !node_ids.insert(node.id()) {
            return Err(BuildError::DuplicateNodeId);
        }

        if node.id() == event.root.root.id {
            has_root = true;
        }
    }

    if !has_root {
        return Err(BuildError::MissingRootNode);
    }

    for node in &event.root.nodes {
        match node {
            EventContentNode::Random(node) => {
                if node.children.is_empty() {
                    return Err(BuildError::EmptyContainer);
                }
            }
            EventContentNode::Sequence(node) | EventContentNode::Layer(node) => {
                if node.children.is_empty() {
                    return Err(BuildError::EmptyContainer);
                }
            }
            EventContentNode::Switch(node) => {
                if node.cases.is_empty() {
                    return Err(BuildError::EmptyContainer);
                }

                validate_ref_set(
                    node.cases.iter().map(|case| case.child),
                    &node_ids,
                    BuildError::MissingChildNode,
                )?;

                if let Some(default_case) = node.default_case {
                    validate_ref(default_case, &node_ids, BuildError::MissingChildNode)?;
                }
            }
            EventContentNode::Loop(node) => {
                validate_ref(node.child, &node_ids, BuildError::MissingChildNode)?;
            }
            EventContentNode::Sampler(_) => {}
        }
    }

    Ok(())
}

pub(crate) fn validate_event_against_parameters(
    event: &Event,
    parameter_by_id: &HashMap<ParameterId, &Parameter>,
) -> Result<(), BuildError> {
    validate_event(event)?;

    for node in &event.root.nodes {
        let EventContentNode::Switch(node) = node else {
            continue;
        };

        let parameter = parameter_by_id
            .get(&node.parameter_id)
            .ok_or(BuildError::MissingParameterDefinition)?;
        let Parameter::Enum(parameter) = parameter else {
            return Err(BuildError::SwitchParameterNotEnum);
        };

        for case in &node.cases {
            if !parameter
                .variants
                .iter()
                .any(|variant| variant == &case.variant)
            {
                return Err(BuildError::UnknownSwitchVariant);
            }
        }
    }

    Ok(())
}

/// 收集一个事件中所有被 `Sampler` 引用的资源 ID
pub fn collect_event_asset_ids(event: &Event) -> HashSet<Uuid> {
    event
        .root
        .nodes
        .iter()
        .filter_map(|node| match node {
            EventContentNode::Sampler(node) => Some(node.asset_id),
            EventContentNode::Random(_)
            | EventContentNode::Sequence(_)
            | EventContentNode::Layer(_)
            | EventContentNode::Switch(_)
            | EventContentNode::Loop(_) => None,
        })
        .collect()
}

#[derive(Default)]
pub(crate) struct MusicGraphDependencies {
    pub(crate) clip_ids: Vec<ClipId>,
    pub(crate) resume_slot_ids: Vec<ResumeSlotId>,
    pub(crate) sync_domain_ids: Vec<SyncDomainId>,
}

pub(crate) fn validate_music_graph(
    graph: &MusicGraph,
    clip_by_id: &HashMap<ClipId, &Clip>,
    resume_slot_by_id: &HashMap<ResumeSlotId, &ResumeSlot>,
    sync_domain_by_id: &HashMap<SyncDomainId, &SyncDomain>,
) -> Result<MusicGraphDependencies, BuildError> {
    if graph.nodes.is_empty() {
        return Err(BuildError::EmptyMusicGraph);
    }

    let mut node_ids = HashSet::new();
    let mut track_ids = HashSet::new();
    let mut track_group_ids = HashSet::new();
    let mut dependencies = MusicGraphDependencies::default();

    for group in &graph.groups {
        if !track_group_ids.insert(group.id) {
            return Err(BuildError::DuplicateTrackGroupId);
        }
    }

    for track in &graph.tracks {
        if !track_ids.insert(track.id) {
            return Err(BuildError::DuplicateTrackId);
        }
        if let Some(group_id) = track.group {
            if !track_group_ids.contains(&group_id) {
                return Err(BuildError::MissingTrackGroupDefinition);
            }
        }
    }

    for node in &graph.nodes {
        if !node_ids.insert(node.id) {
            return Err(BuildError::DuplicateMusicNodeId);
        }

        if node.bindings.is_empty() {
            return Err(BuildError::EmptyMusicNode);
        }

        if let Some(memory_slot) = node.memory_slot {
            if !resume_slot_by_id.contains_key(&memory_slot) {
                return Err(BuildError::MissingResumeSlotDefinition);
            }
            push_unique(&mut dependencies.resume_slot_ids, memory_slot);
        }

        let mut state_binding_track_ids = HashSet::<TrackId>::new();
        for binding in &node.bindings {
            if !track_ids.contains(&binding.track_id) {
                return Err(BuildError::MissingTrackDefinition);
            }
            if !state_binding_track_ids.insert(binding.track_id) {
                return Err(BuildError::DuplicateTrackBinding);
            }

            for clip_id in binding.target.clip_ids() {
                let clip = clip_by_id
                    .get(&clip_id)
                    .ok_or(BuildError::MissingClipDefinition)?;
                push_unique(&mut dependencies.clip_ids, clip_id);

                if let Some(sync_domain_id) = clip.sync_domain {
                    if !sync_domain_by_id.contains_key(&sync_domain_id) {
                        return Err(BuildError::MissingSyncDomainDefinition);
                    }
                    push_unique(&mut dependencies.sync_domain_ids, sync_domain_id);
                }
            }
        }

        if let Some(completion_source) = node.completion_source {
            if !node
                .bindings
                .iter()
                .any(|binding| binding.track_id == completion_source)
            {
                return Err(BuildError::MissingCompletionTrackBinding);
            }
        }
    }

    if let Some(initial_node) = graph.initial_node {
        if !node_ids.contains(&initial_node) {
            return Err(BuildError::MissingMusicNodeDefinition);
        }
    }

    for edge in &graph.edges {
        if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
            return Err(BuildError::MissingMusicNodeDefinition);
        }
        if let Some(requested_target) = edge.requested_target {
            if !node_ids.contains(&requested_target) {
                return Err(BuildError::MissingMusicNodeDefinition);
            }
        }
    }

    Ok(dependencies)
}

pub(crate) fn push_unique<T: PartialEq + Copy>(items: &mut Vec<T>, value: T) {
    if !items.contains(&value) {
        items.push(value);
    }
}

fn validate_ref(
    node_ref: NodeRef,
    node_ids: &HashSet<NodeId>,
    error: BuildError,
) -> Result<(), BuildError> {
    if node_ids.contains(&node_ref.id) {
        Ok(())
    } else {
        Err(error)
    }
}

fn validate_ref_set(
    refs: impl IntoIterator<Item = NodeRef>,
    node_ids: &HashSet<NodeId>,
    error: BuildError,
) -> Result<(), BuildError> {
    for node_ref in refs {
        validate_ref(node_ref, node_ids, error)?;
    }

    Ok(())
}
