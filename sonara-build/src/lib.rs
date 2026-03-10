//! Sonara 的构建层
//!
//! 这一层负责 authoring 数据校验和 bank 构建

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sonara_model::{
    AudioAsset, AuthoringProject, Bank, BankAsset, BankDefinition, Bus, Event, EventContentNode,
    EventId, NodeId, NodeRef, Snapshot, SnapshotId, StreamingMode,
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
    #[error("bank 定义引用了不存在的 bus")]
    MissingBusDefinition,
    #[error("bank 定义引用了不存在的 snapshot")]
    MissingSnapshotDefinition,
}

/// compiled bank 文件的最小 IO 错误。
#[derive(Debug, Error)]
pub enum CompiledBankFileError {
    #[error("读取 compiled bank 文件失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("compiled bank JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),
}

/// 一次 bank 编译后的最小载荷。
///
/// 它把 runtime/backend 加载一个 compiled bank 所需的高层对象定义放在一起，
/// 便于后续从文件读取后直接进入加载流程。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledBankPackage {
    pub bank: Bank,
    pub events: Vec<Event>,
    pub buses: Vec<Bus>,
    pub snapshots: Vec<Snapshot>,
}

impl CompiledBankPackage {
    /// 从 JSON 字符串读取 compiled bank 载荷。
    pub fn from_json_str(contents: &str) -> Result<Self, CompiledBankFileError> {
        Ok(serde_json::from_str(contents)?)
    }

    /// 把 compiled bank 载荷编码成格式化 JSON。
    pub fn to_json_string_pretty(&self) -> Result<String, CompiledBankFileError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// 从磁盘读取一个 JSON compiled bank 文件。
    pub fn read_json_file(path: impl AsRef<Path>) -> Result<Self, CompiledBankFileError> {
        let contents = fs::read_to_string(path)?;
        Self::from_json_str(&contents)
    }

    /// 把 compiled bank 载荷写到磁盘上的 JSON 文件。
    pub fn write_json_file(&self, path: impl AsRef<Path>) -> Result<(), CompiledBankFileError> {
        let contents = self.to_json_string_pretty()?;
        fs::write(path, contents)?;
        Ok(())
    }
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
        bank.objects.events.push(event.id);

        for asset_id in collect_event_asset_ids(event) {
            let asset = asset_by_id
                .get(&asset_id)
                .ok_or(BuildError::MissingAudioAsset)?;

            if !bank
                .manifest
                .assets
                .iter()
                .any(|bank_asset| bank_asset.id == asset_id)
            {
                bank.manifest.assets.push(BankAsset {
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

    bank.manifest.resident_media = resident_media.into_iter().collect();
    bank.manifest.streaming_media = streaming_media.into_iter().collect();
    bank.manifest.assets.sort_by(|a, b| a.id.cmp(&b.id));
    bank.manifest.resident_media.sort_unstable();
    bank.manifest.streaming_media.sort_unstable();

    Ok(bank)
}

/// 根据 authoring 项目里的 bank 定义构建一个 runtime bank。
pub fn build_bank_from_definition(
    definition: &BankDefinition,
    project: &AuthoringProject,
) -> Result<Bank, BuildError> {
    Ok(compile_bank_definition(definition, project)?.bank)
}

/// 根据 authoring 项目里的 bank 定义编译一份完整 bank 载荷。
pub fn compile_bank_definition(
    definition: &BankDefinition,
    project: &AuthoringProject,
) -> Result<CompiledBankPackage, BuildError> {
    let event_by_id: HashMap<EventId, &Event> = project
        .events
        .iter()
        .map(|event| (event.id, event))
        .collect();
    let bus_by_id: HashMap<_, &Bus> = project.buses.iter().map(|bus| (bus.id, bus)).collect();
    let snapshot_by_id: HashMap<SnapshotId, &Snapshot> = project
        .snapshots
        .iter()
        .map(|snapshot| (snapshot.id, snapshot))
        .collect();

    let mut events = Vec::with_capacity(definition.events.len());
    let mut buses = Vec::with_capacity(definition.buses.len());
    let mut snapshots = Vec::with_capacity(definition.snapshots.len());

    for event_id in &definition.events {
        let event = event_by_id
            .get(event_id)
            .ok_or(BuildError::MissingEventDefinition)?;
        events.push((*event).clone());
    }

    for bus_id in &definition.buses {
        let bus = bus_by_id
            .get(bus_id)
            .ok_or(BuildError::MissingBusDefinition)?;
        buses.push((*bus).clone());
    }

    for snapshot_id in &definition.snapshots {
        let snapshot = snapshot_by_id
            .get(snapshot_id)
            .ok_or(BuildError::MissingSnapshotDefinition)?;
        snapshots.push((*snapshot).clone());
    }

    let mut bank = build_bank(definition.name.clone(), &events, &project.assets)?;
    bank.id = definition.id;
    bank.objects.buses = definition.buses.clone();
    bank.objects.snapshots = definition.snapshots.clone();

    Ok(CompiledBankPackage {
        bank,
        events,
        buses,
        snapshots,
    })
}

/// 根据 authoring 项目里的 bank 定义编译并写出一份 compiled bank 文件。
///
/// 这条路径用于把 editor/authoring 层维护的 project 数据导出为 runtime 可直接加载的产物。
pub fn compile_bank_definition_to_file(
    definition: &BankDefinition,
    project: &AuthoringProject,
    output_path: impl AsRef<Path>,
) -> Result<CompiledBankPackage, ExportBankError> {
    let package = compile_bank_definition(definition, project)?;
    package.write_json_file(output_path)?;
    Ok(package)
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

/// compiled bank 导出阶段错误。
#[derive(Debug, Error)]
pub enum ExportBankError {
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error(transparent)]
    File(#[from] CompiledBankFileError),
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
        assert_eq!(bank.objects.events.len(), 1);
        assert_eq!(bank.manifest.assets.len(), 2);
        assert_eq!(bank.manifest.resident_media, vec![resident_asset.id]);
        assert_eq!(bank.manifest.streaming_media, vec![streaming_asset.id]);
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
        let manifest_asset = bank
            .manifest
            .assets
            .first()
            .expect("manifest asset should exist");

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
        assert_eq!(bank.objects.events, vec![selected_event.id]);
        assert_eq!(bank.manifest.assets.len(), 1);
        assert_eq!(bank.manifest.assets[0].id, selected_asset.id);
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

    #[test]
    fn build_bank_from_definition_preserves_bus_and_snapshot_selection() {
        let mut project = AuthoringProject::new("demo");
        let bus = sonara_model::Bus::new("sfx");
        let snapshot = sonara_model::Snapshot {
            id: sonara_model::SnapshotId::new(),
            name: "combat".into(),
            fade_in_seconds: 0.2,
            fade_out_seconds: 0.4,
            targets: Vec::new(),
        };
        project.buses.push(bus.clone());
        project.snapshots.push(snapshot.clone());

        let mut definition = sonara_model::BankDefinition::new("core");
        definition.buses.push(bus.id);
        definition.snapshots.push(snapshot.id);

        let bank = build_bank_from_definition(&definition, &project)
            .expect("bank should build from project");

        assert_eq!(bank.objects.buses, vec![bus.id]);
        assert_eq!(bank.objects.snapshots, vec![snapshot.id]);
    }

    #[test]
    fn compile_bank_definition_returns_selected_object_definitions() {
        let selected_asset = make_asset("footstep_wood_01", StreamingMode::Resident);
        let selected_sampler_id = NodeId::new();
        let selected_event = make_event(
            vec![EventContentNode::Sampler(SamplerNode {
                id: selected_sampler_id,
                asset_id: selected_asset.id,
            })],
            selected_sampler_id,
        );
        let bus = sonara_model::Bus::new("sfx");
        let snapshot = sonara_model::Snapshot {
            id: sonara_model::SnapshotId::new(),
            name: "combat".into(),
            fade_in_seconds: 0.2,
            fade_out_seconds: 0.4,
            targets: vec![sonara_model::SnapshotTarget {
                bus_id: bus.id,
                target_volume: 0.8,
            }],
        };

        let mut project = AuthoringProject::new("demo");
        project.assets.push(selected_asset);
        project.events.push(selected_event.clone());
        project.buses.push(bus.clone());
        project.snapshots.push(snapshot.clone());

        let mut definition = sonara_model::BankDefinition::new("core");
        definition.events.push(selected_event.id);
        definition.buses.push(bus.id);
        definition.snapshots.push(snapshot.id);

        let package =
            compile_bank_definition(&definition, &project).expect("package should compile");

        assert_eq!(package.bank.id, definition.id);
        assert_eq!(package.events, vec![selected_event]);
        assert_eq!(package.buses, vec![bus]);
        assert_eq!(package.snapshots, vec![snapshot]);
    }

    #[test]
    fn compiled_bank_package_json_round_trip_preserves_bank_name() {
        let package = CompiledBankPackage {
            bank: Bank::new("core"),
            events: Vec::new(),
            buses: Vec::new(),
            snapshots: Vec::new(),
        };

        let json = package
            .to_json_string_pretty()
            .expect("compiled package should serialize");
        let decoded = CompiledBankPackage::from_json_str(&json)
            .expect("compiled package should deserialize from JSON");

        assert_eq!(decoded.bank.name, "core");
    }

    #[test]
    fn compile_bank_definition_to_file_writes_compiled_bank_json() {
        let asset = make_asset("footstep_wood_01", StreamingMode::Resident);
        let sampler_id = NodeId::new();
        let event = make_event(
            vec![EventContentNode::Sampler(SamplerNode {
                id: sampler_id,
                asset_id: asset.id,
            })],
            sampler_id,
        );

        let mut project = AuthoringProject::new("demo");
        project.assets.push(asset);
        project.events.push(event.clone());

        let mut definition = sonara_model::BankDefinition::new("core");
        definition.events.push(event.id);

        let output_path =
            std::env::temp_dir().join(format!("sonara-compiled-bank-{}.json", Uuid::now_v7()));
        let package = compile_bank_definition_to_file(&definition, &project, &output_path)
            .expect("compiled bank export should succeed");
        let decoded = CompiledBankPackage::read_json_file(&output_path)
            .expect("exported compiled bank file should be readable");

        assert_eq!(decoded.bank.id, package.bank.id);
        assert_eq!(decoded.bank.name, "core");

        std::fs::remove_file(output_path).expect("temp compiled bank file should be removed");
    }
}
