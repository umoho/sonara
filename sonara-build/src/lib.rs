//! Sonara 的构建层
//!
//! 这一层负责 authoring 数据校验和 bank 构建

use sonara_model::{Event, EventContentNode};
use thiserror::Error;

/// 构建阶段错误
#[derive(Debug, Error)]
pub enum BuildError {
    #[error("事件内容树为空")]
    EmptyEventTree,
    #[error("事件根节点不存在")]
    MissingRootNode,
    #[error("容器节点必须至少包含一个子节点")]
    EmptyContainer,
}

/// 对单个事件做最小语义校验
pub fn validate_event(event: &Event) -> Result<(), BuildError> {
    if event.root.nodes.is_empty() {
        return Err(BuildError::EmptyEventTree);
    }

    let has_root = event
        .root
        .nodes
        .iter()
        .any(|node| node.id() == event.root.root.id);

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
            }
            EventContentNode::Loop(_) | EventContentNode::Sampler(_) => {}
        }
    }

    Ok(())
}
