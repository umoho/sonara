//! 编辑器 UI 层骨架

use sonara_model::Event;

/// 编辑器运行时状态
#[derive(Debug, Default)]
pub struct EditorState {
    pub selected_event: Option<Event>,
}
