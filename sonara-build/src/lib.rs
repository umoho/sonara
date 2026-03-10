//! Sonara 的构建层
//!
//! 这一层负责 authoring 数据校验和 bank 构建

use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;
use sonara_model::{
    AudioAsset, AuthoringProject, Bank, BankAsset, BankDefinition, Event, EventContentNode,
    EventId, NodeId, NodeRef, StreamingMode,
};
use thiserror::Error;
use uuid::Uuid;

/// 构建阶段错误
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BuildError {
    #[error("事件内容树为空")]
    EmptyEventTree,
    #[error("事件根节点不存在")]
    MissingRootNode,
    #[error("事件内容树存在重复节点 ID")]
    DuplicateNodeId,
    #[error("节点引用了不存在的子节点")]
    MissingChildNode,
    #[error("容器节点必须至少包含一个子节点")]
    EmptyContainer,
    #[error("事件引用了不存在的音频资源")]
    MissingAudioAsset,
    #[error("bank 定义引用了不存在的事件")]
    MissingEventDefinition,
}

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

/// 根据事件和资源列表构建最小 bank 定义
pub fn build_bank(
    name: impl Into<SmolStr>,
    events: &[Event],
    assets: &[AudioAsset],
) -> Result<Bank, BuildError> {
    let mut bank = Bank::new(name);
    let asset_by_id: HashMap<Uuid, &AudioAsset> =
        assets.iter().map(|asset| (asset.id, asset)).collect();
    let mut resident_media = HashSet::new();
    let mut streaming_media = HashSet::new();

    for event in events {
        validate_event(event)?;
        bank.events.push(event.id);

        for asset_id in collect_event_asset_ids(event) {
            let asset = asset_by_id
                .get(&asset_id)
                .ok_or(BuildError::MissingAudioAsset)?;

            if !bank
                .assets
                .iter()
                .any(|bank_asset| bank_asset.id == asset_id)
            {
                bank.assets.push(BankAsset {
                    id: asset.id,
                    name: asset.name.clone(),
                    source_path: asset.source_path.clone(),
                    import_settings: asset.import_settings.clone(),
                    streaming: asset.streaming,
                });
            }

            match asset.streaming {
                StreamingMode::Auto | StreamingMode::Resident => {
                    resident_media.insert(asset_id);
                }
                StreamingMode::Streaming => {
                    streaming_media.insert(asset_id);
                }
            }
        }
    }

    bank.resident_media = resident_media.into_iter().collect();
    bank.streaming_media = streaming_media.into_iter().collect();
    bank.assets.sort_by(|a, b| a.id.cmp(&b.id));
    bank.resident_media.sort_unstable();
    bank.streaming_media.sort_unstable();

    Ok(bank)
}

/// 根据 authoring 项目里的 bank 定义构建一个 runtime bank。
pub fn build_bank_from_definition(
    definition: &BankDefinition,
    project: &AuthoringProject,
) -> Result<Bank, BuildError> {
    let event_by_id: HashMap<EventId, &Event> = project
        .events
        .iter()
        .map(|event| (event.id, event))
        .collect();
    let mut events = Vec::with_capacity(definition.events.len());

    for event_id in &definition.events {
        let event = event_by_id
            .get(event_id)
            .ok_or(BuildError::MissingEventDefinition)?;
        events.push((*event).clone());
    }

    let mut bank = build_bank(definition.name.clone(), &events, &project.assets)?;
    bank.id = definition.id;
    Ok(bank)
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

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use sonara_model::{
        AuthoringProject, EventContentRoot, EventId, EventKind, ParameterId, SamplerNode,
        SequenceNode, SpatialMode, SwitchCase, SwitchNode,
    };

    use super::*;

    fn make_event(nodes: Vec<EventContentNode>, root: NodeId) -> Event {
        Event {
            id: EventId::new(),
            name: "player.footstep".into(),
            kind: EventKind::OneShot,
            root: EventContentRoot {
                root: NodeRef { id: root },
                nodes,
            },
            default_bus: None,
            spatial: SpatialMode::ThreeD,
            default_parameters: Vec::new(),
            voice_limit: None,
            steal_policy: None,
        }
    }

    fn make_asset(name: &str, streaming: StreamingMode) -> AudioAsset {
        let mut asset = AudioAsset::new(name, Utf8PathBuf::from(format!("audio/{name}.wav")));
        asset.streaming = streaming;
        asset
    }

    #[test]
    fn validate_event_rejects_missing_root_node() {
        let sampler_id = NodeId::new();
        let event = make_event(
            vec![EventContentNode::Sampler(SamplerNode {
                id: sampler_id,
                asset_id: Uuid::now_v7(),
            })],
            NodeId::new(),
        );

        assert!(matches!(
            validate_event(&event),
            Err(BuildError::MissingRootNode)
        ));
    }

    #[test]
    fn validate_event_rejects_missing_child_reference() {
        let switch_id = NodeId::new();
        let missing_child = NodeId::new();
        let event = make_event(
            vec![EventContentNode::Switch(SwitchNode {
                id: switch_id,
                parameter_id: ParameterId::new(),
                cases: vec![SwitchCase {
                    variant: "wood".into(),
                    child: NodeRef { id: missing_child },
                }],
                default_case: None,
            })],
            switch_id,
        );

        assert!(matches!(
            validate_event(&event),
            Err(BuildError::MissingChildNode)
        ));
    }

    #[test]
    fn build_bank_collects_resident_and_streaming_media() {
        let resident_asset = make_asset("footstep_wood_01", StreamingMode::Resident);
        let streaming_asset = make_asset("music_forest", StreamingMode::Streaming);
        let sampler_a = NodeId::new();
        let sampler_b = NodeId::new();
        let root_id = NodeId::new();

        let event = make_event(
            vec![
                EventContentNode::Sequence(SequenceNode {
                    id: root_id,
                    children: vec![NodeRef { id: sampler_a }, NodeRef { id: sampler_b }],
                }),
                EventContentNode::Sampler(SamplerNode {
                    id: sampler_a,
                    asset_id: resident_asset.id,
                }),
                EventContentNode::Sampler(SamplerNode {
                    id: sampler_b,
                    asset_id: streaming_asset.id,
                }),
            ],
            root_id,
        );

        let bank = build_bank(
            "core",
            &[event],
            &[resident_asset.clone(), streaming_asset.clone()],
        )
        .expect("bank should build");

        assert_eq!(bank.name.as_str(), "core");
        assert_eq!(bank.events.len(), 1);
        assert_eq!(bank.assets.len(), 2);
        assert_eq!(bank.resident_media, vec![resident_asset.id]);
        assert_eq!(bank.streaming_media, vec![streaming_asset.id]);
    }

    #[test]
    fn build_bank_preserves_asset_import_settings_in_manifest() {
        let sampler_id = NodeId::new();
        let mut asset = make_asset("footstep_wood_01", StreamingMode::Resident);
        asset.import_settings.normalize = true;
        asset.import_settings.target_sample_rate = Some(48_000);
        let event = make_event(
            vec![EventContentNode::Sampler(SamplerNode {
                id: sampler_id,
                asset_id: asset.id,
            })],
            sampler_id,
        );

        let bank = build_bank("core", &[event], &[asset.clone()]).expect("bank should build");
        let manifest_asset = bank.assets.first().expect("manifest asset should exist");

        assert_eq!(manifest_asset.id, asset.id);
        assert_eq!(manifest_asset.import_settings, asset.import_settings);
    }

    #[test]
    fn build_bank_rejects_missing_asset() {
        let sampler_id = NodeId::new();
        let asset_id = Uuid::now_v7();
        let event = make_event(
            vec![EventContentNode::Sampler(SamplerNode {
                id: sampler_id,
                asset_id,
            })],
            sampler_id,
        );

        assert!(matches!(
            build_bank("core", &[event], &[]),
            Err(BuildError::MissingAudioAsset)
        ));
    }

    #[test]
    fn build_bank_from_definition_uses_project_event_selection() {
        let selected_asset = make_asset("footstep_wood_01", StreamingMode::Resident);
        let ignored_asset = make_asset("ui_click", StreamingMode::Resident);
        let selected_sampler_id = NodeId::new();
        let ignored_sampler_id = NodeId::new();

        let selected_event = make_event(
            vec![EventContentNode::Sampler(SamplerNode {
                id: selected_sampler_id,
                asset_id: selected_asset.id,
            })],
            selected_sampler_id,
        );
        let ignored_event = make_event(
            vec![EventContentNode::Sampler(SamplerNode {
                id: ignored_sampler_id,
                asset_id: ignored_asset.id,
            })],
            ignored_sampler_id,
        );

        let mut project = AuthoringProject::new("demo");
        project.assets.push(selected_asset.clone());
        project.assets.push(ignored_asset);
        project.events.push(selected_event.clone());
        project.events.push(ignored_event);

        let mut definition = sonara_model::BankDefinition::new("core");
        definition.events.push(selected_event.id);

        let bank = build_bank_from_definition(&definition, &project)
            .expect("bank should build from project");

        assert_eq!(bank.id, definition.id);
        assert_eq!(bank.events, vec![selected_event.id]);
        assert_eq!(bank.assets.len(), 1);
        assert_eq!(bank.assets[0].id, selected_asset.id);
    }

    #[test]
    fn build_bank_from_definition_rejects_missing_project_event() {
        let project = AuthoringProject::new("demo");
        let mut definition = sonara_model::BankDefinition::new("core");
        definition.events.push(EventId::new());

        assert!(matches!(
            build_bank_from_definition(&definition, &project),
            Err(BuildError::MissingEventDefinition)
        ));
    }
}
