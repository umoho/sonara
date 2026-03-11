//! Sonara 编辑器最小壳子。
//!
//! 当前阶段只打通 authoring 项目读取和 compiled bank 导出流程。

mod content;
mod i18n;
mod ui;

use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::content::{AssetOption, EnumParameterOption, parse_variant_list};
use eframe::egui;
use egui_chinese_font::setup_chinese_fonts;
use i18n::{EditorLocale, TextKey, TextTemplate, template, text};
use sonara_build::{
    BuildError, ProjectExportBankError, compile_bank_definition, compile_project_bank_to_file,
};
use sonara_model::{
    AuthoringProject, BankDefinition, Bus, EnumParameter, Event, EventContentNode,
    EventContentRoot, EventKind, NodeId, NodeRef, Parameter, ParameterScope, ProjectFileError,
    SamplerNode, Snapshot, SpatialMode,
};

/// 默认打开的 demo project 路径。
pub const DEFAULT_PROJECT_PATH: &str = "sonara-app/assets/demo/project.json";

/// 启动编辑器窗口。
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Sonara Editor")
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Sonara Editor",
        options,
        Box::new(|cc| {
            let mut app = EditorApp::new();

            if let Err(error) = setup_chinese_fonts(&cc.egui_ctx) {
                app.state.status_message = app.state.tr(TextTemplate::FontLoadFailed {
                    error: error.to_string(),
                });
                app.state
                    .push_error_log(app.state.tr(TextTemplate::FontLoadFailed {
                        error: error.to_string(),
                    }));
            } else {
                app.state
                    .push_info_log(app.state.tx(TextKey::FontLoaded).to_owned());
            }

            Ok(Box::new(app))
        }),
    )
}

/// 最小编辑器应用。
pub struct EditorApp {
    state: EditorState,
}

impl EditorApp {
    /// 创建一个带默认 demo 路径的编辑器实例。
    pub fn new() -> Self {
        let mut state = EditorState::default();
        state.project_path = DEFAULT_PROJECT_PATH.to_owned();
        state.load_project();
        Self { state }
    }
}

impl Default for EditorApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.draw_menu_bar(ctx);
        self.state.draw_status_bar(ctx);
        self.state.draw_tool_strip(ctx);
        self.state.draw_left_panel(ctx);
        self.state.draw_right_panel(ctx);
        self.state.draw_center_panel(ctx);
        self.state.draw_open_project_window(ctx);
        self.state.draw_export_bank_window(ctx);
        self.state.draw_bank_events_window(ctx);
        self.state.draw_diagnostics_window(ctx);
        self.state.draw_log_window(ctx);
        self.state.draw_about_window(ctx);
    }
}

/// 编辑器运行时状态。
///
/// 这一层只维护 UI 所需的瞬时状态, 不把 authoring 模型和 UI 容器硬耦合。
#[derive(Debug)]
pub struct EditorState {
    pub locale: EditorLocale,
    pub project_path: String,
    pub export_path: String,
    pub recent_projects: Vec<String>,
    pub loaded_project: Option<AuthoringProject>,
    pub selected_bank_name: Option<String>,
    pub selected_item: Option<SelectedItem>,
    pub validation_report: ValidationReport,
    pub last_export: Option<ExportReport>,
    pub has_unsaved_changes: bool,
    pub show_project_panel: bool,
    pub show_inspector_panel: bool,
    pub show_tool_strip: bool,
    pub show_status_bar: bool,
    pub show_open_project_window: bool,
    pub show_export_bank_window: bool,
    pub show_bank_events_window: bool,
    pub show_diagnostics_window: bool,
    pub show_log_window: bool,
    pub show_about_window: bool,
    pub new_event_name: String,
    pub new_bus_name: String,
    pub new_snapshot_name: String,
    pub new_parameter_name: String,
    pub new_parameter_variants: String,
    pub status_message: String,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedItem {
    Event(sonara_model::EventId),
    Bus(sonara_model::BusId),
    Snapshot(sonara_model::SnapshotId),
    Parameter(sonara_model::ParameterId),
}

/// 编辑器日志级别。
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Error,
}

/// 编辑器底部日志条目。
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

/// 最近一次导出的摘要。
#[derive(Debug, Clone)]
pub struct ExportReport {
    pub success: bool,
    pub bank_name: String,
    pub output_path: String,
    pub event_count: usize,
    pub bus_count: usize,
    pub snapshot_count: usize,
    pub asset_count: usize,
    pub resident_media_count: usize,
    pub streaming_media_count: usize,
    pub file_size_bytes: Option<u64>,
    pub error_message: Option<String>,
}

/// 当前选中 Bank 的导出前校验结果。
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub issues: Vec<String>,
    pub asset_count: Option<usize>,
    pub resident_media_count: Option<usize>,
    pub streaming_media_count: Option<usize>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            locale: EditorLocale::default(),
            project_path: String::new(),
            export_path: String::new(),
            recent_projects: Vec::new(),
            loaded_project: None,
            selected_bank_name: None,
            selected_item: None,
            validation_report: ValidationReport::default(),
            last_export: None,
            has_unsaved_changes: false,
            show_project_panel: true,
            show_inspector_panel: true,
            show_tool_strip: true,
            show_status_bar: true,
            show_open_project_window: false,
            show_export_bank_window: false,
            show_bank_events_window: false,
            show_diagnostics_window: false,
            show_log_window: false,
            show_about_window: false,
            new_event_name: String::new(),
            new_bus_name: String::new(),
            new_snapshot_name: String::new(),
            new_parameter_name: String::new(),
            new_parameter_variants: String::new(),
            status_message: String::new(),
            logs: Vec::new(),
        }
    }
}

impl EditorState {
    fn tx(&self, key: TextKey) -> &'static str {
        text(self.locale, key)
    }

    fn tr(&self, template_value: TextTemplate) -> String {
        template(self.locale, template_value)
    }

    fn asset_options(&self) -> Vec<AssetOption> {
        self.loaded_project
            .as_ref()
            .map(|project| {
                project
                    .assets
                    .iter()
                    .map(|asset| AssetOption {
                        id: asset.id,
                        name: asset.name.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn enum_parameter_options(&self) -> Vec<EnumParameterOption> {
        self.loaded_project
            .as_ref()
            .map(|project| {
                project
                    .parameters
                    .iter()
                    .filter_map(|parameter| {
                        let Parameter::Enum(parameter) = parameter else {
                            return None;
                        };

                        Some(EnumParameterOption {
                            id: parameter.id,
                            name: parameter.name.to_string(),
                            default_value: parameter.default_value.to_string(),
                            variants: parameter
                                .variants
                                .iter()
                                .map(|value| value.to_string())
                                .collect(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 加载当前路径指向的 project 文件。
    pub fn load_project(&mut self) {
        match AuthoringProject::read_json_file(&self.project_path) {
            Ok(project) => {
                self.loaded_project = Some(project);
                self.last_export = None;
                self.has_unsaved_changes = false;
                self.push_recent_project(self.project_path.clone());
                self.refresh_validation();
                self.status_message = self.tr(TextTemplate::ProjectLoaded {
                    path: self.project_path.clone(),
                });

                let first_bank_name = self
                    .loaded_project
                    .as_ref()
                    .and_then(|project| project.banks.first())
                    .map(|bank| bank.name.to_string());
                self.selected_bank_name = first_bank_name.clone();
                self.export_path = self
                    .selected_bank_name
                    .as_deref()
                    .map(|name| self.suggest_export_path(name))
                    .unwrap_or_default();

                self.push_info_log(self.tr(TextTemplate::LoadSucceeded {
                    path: self.project_path.clone(),
                }));
            }
            Err(error) => {
                self.loaded_project = None;
                self.selected_bank_name = None;
                self.validation_report = ValidationReport::default();
                self.has_unsaved_changes = false;
                self.status_message = self.tr(TextTemplate::LoadFailed {
                    error: render_project_error(&error),
                });
                self.push_error_log(self.tr(TextTemplate::LoadFailedLog {
                    path: self.project_path.clone(),
                    error: render_project_error(&error),
                }));
            }
        }
    }

    /// 选择当前项目里的一个 bank。
    pub fn select_bank(&mut self, bank_name: &str) {
        self.selected_bank_name = Some(bank_name.to_owned());
        self.selected_item = None;
        self.export_path = self.suggest_export_path(bank_name);
        self.refresh_validation();
        self.status_message = self.tr(TextTemplate::SelectBank {
            bank_name: bank_name.to_owned(),
        });
    }

    /// 保存当前项目文件。
    pub fn save_project(&mut self) {
        let Some(project) = &self.loaded_project else {
            self.status_message = self.tr(TextTemplate::SaveFailed {
                error: "尚未加载项目".to_owned(),
            });
            return;
        };

        match project.write_json_file(&self.project_path) {
            Ok(()) => {
                self.has_unsaved_changes = false;
                self.status_message = self.tr(TextTemplate::SaveSucceeded {
                    path: self.project_path.clone(),
                });
                self.push_info_log(self.tr(TextTemplate::SaveSucceeded {
                    path: self.project_path.clone(),
                }));
            }
            Err(error) => {
                let rendered_error = render_project_error(&error);
                self.status_message = self.tr(TextTemplate::SaveFailed {
                    error: rendered_error.clone(),
                });
                self.push_error_log(self.tr(TextTemplate::SaveFailedLog {
                    path: self.project_path.clone(),
                    error: rendered_error,
                }));
            }
        }
    }

    /// 导出当前选中的 bank。
    pub fn export_selected_bank(&mut self) {
        let Some(project) = &self.loaded_project else {
            self.status_message = self.tr(TextTemplate::ExportFailedNoProject);
            self.push_error_log(self.tr(TextTemplate::ExportFailedNoProject));
            return;
        };

        let Some(bank_name) = self.selected_bank_name.clone() else {
            self.status_message = self.tr(TextTemplate::ExportFailedNoBank);
            self.push_error_log(self.tr(TextTemplate::ExportFailedNoBank));
            return;
        };

        if self.export_path.trim().is_empty() {
            self.status_message = self.tr(TextTemplate::ExportFailedEmptyOutputPath);
            self.push_error_log(self.tr(TextTemplate::ExportFailedEmptyOutputPath));
            return;
        }

        match compile_project_bank_to_file(project, &bank_name, &self.export_path) {
            Ok(package) => {
                let file_size_bytes = fs::metadata(&self.export_path)
                    .ok()
                    .map(|metadata| metadata.len());
                self.last_export = Some(ExportReport::success(
                    package.bank.name.to_string(),
                    self.export_path.clone(),
                    package.events.len(),
                    package.buses.len(),
                    package.snapshots.len(),
                    package.bank.manifest.assets.len(),
                    package.bank.manifest.resident_media.len(),
                    package.bank.manifest.streaming_media.len(),
                    file_size_bytes,
                ));
                self.status_message = self.tr(TextTemplate::ExportSucceeded {
                    bank_name: package.bank.name.to_string(),
                    output_path: self.export_path.clone(),
                });
                self.push_info_log(self.tr(TextTemplate::ExportSucceededLog {
                    bank_name: package.bank.name.to_string(),
                    event_count: package.events.len(),
                    output_path: self.export_path.clone(),
                }));
            }
            Err(error) => {
                let rendered_error = render_export_error(&error);
                self.last_export = Some(ExportReport::failure(
                    bank_name.clone(),
                    self.export_path.clone(),
                    rendered_error.clone(),
                ));
                self.status_message = self.tr(TextTemplate::ExportFailed {
                    error: rendered_error.clone(),
                });
                self.push_error_log(self.tr(TextTemplate::ExportFailedLog {
                    bank_name,
                    output_path: self.export_path.clone(),
                    error: rendered_error,
                }));
            }
        }
    }

    fn selected_bank<'a>(&self, project: &'a AuthoringProject) -> Option<&'a BankDefinition> {
        let bank_name = self.selected_bank_name.as_deref()?;
        project.bank_named(bank_name)
    }

    fn suggest_export_path(&self, bank_name: &str) -> String {
        let project_path = Path::new(self.project_path.trim());

        if let Some(parent) = project_path.parent() {
            return parent
                .join(format!("{bank_name}.bank.json"))
                .display()
                .to_string();
        }

        format!("{bank_name}.bank.json")
    }

    fn push_info_log(&mut self, message: String) {
        self.logs.push(LogEntry::new(LogLevel::Info, message));
    }

    fn push_error_log(&mut self, message: String) {
        self.logs.push(LogEntry::new(LogLevel::Error, message));
    }

    fn push_recent_project(&mut self, path: String) {
        self.recent_projects.retain(|item| item != &path);
        self.recent_projects.insert(0, path);
        self.recent_projects.truncate(5);
    }

    fn add_event_to_selected_bank(&mut self, event_id: sonara_model::EventId) {
        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.as_deref() else {
            return;
        };
        let Some(bank) = project.banks.iter_mut().find(|bank| bank.name == bank_name) else {
            return;
        };

        if !bank.events.contains(&event_id) {
            bank.events.push(event_id);
            self.has_unsaved_changes = true;
            self.last_export = None;
            self.refresh_validation();
        }
    }

    fn remove_event_from_selected_bank(&mut self, event_id: sonara_model::EventId) {
        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.as_deref() else {
            return;
        };
        let Some(bank) = project.banks.iter_mut().find(|bank| bank.name == bank_name) else {
            return;
        };

        let original_len = bank.events.len();
        bank.events.retain(|id| *id != event_id);
        if bank.events.len() != original_len {
            if self.selected_item == Some(SelectedItem::Event(event_id)) {
                self.selected_item = None;
            }
            self.has_unsaved_changes = true;
            self.last_export = None;
            self.refresh_validation();
        }
    }

    fn add_bus_to_selected_bank(&mut self, bus_id: sonara_model::BusId) {
        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.as_deref() else {
            return;
        };
        let Some(bank) = project.banks.iter_mut().find(|bank| bank.name == bank_name) else {
            return;
        };

        if !bank.buses.contains(&bus_id) {
            bank.buses.push(bus_id);
            self.has_unsaved_changes = true;
            self.last_export = None;
            self.refresh_validation();
        }
    }

    fn remove_bus_from_selected_bank(&mut self, bus_id: sonara_model::BusId) {
        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.as_deref() else {
            return;
        };
        let Some(bank) = project.banks.iter_mut().find(|bank| bank.name == bank_name) else {
            return;
        };

        let original_len = bank.buses.len();
        bank.buses.retain(|id| *id != bus_id);
        if bank.buses.len() != original_len {
            if self.selected_item == Some(SelectedItem::Bus(bus_id)) {
                self.selected_item = None;
            }
            self.has_unsaved_changes = true;
            self.last_export = None;
            self.refresh_validation();
        }
    }

    fn add_snapshot_to_selected_bank(&mut self, snapshot_id: sonara_model::SnapshotId) {
        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.as_deref() else {
            return;
        };
        let Some(bank) = project.banks.iter_mut().find(|bank| bank.name == bank_name) else {
            return;
        };

        if !bank.snapshots.contains(&snapshot_id) {
            bank.snapshots.push(snapshot_id);
            self.has_unsaved_changes = true;
            self.last_export = None;
            self.refresh_validation();
        }
    }

    fn remove_snapshot_from_selected_bank(&mut self, snapshot_id: sonara_model::SnapshotId) {
        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.as_deref() else {
            return;
        };
        let Some(bank) = project.banks.iter_mut().find(|bank| bank.name == bank_name) else {
            return;
        };

        let original_len = bank.snapshots.len();
        bank.snapshots.retain(|id| *id != snapshot_id);
        if bank.snapshots.len() != original_len {
            if self.selected_item == Some(SelectedItem::Snapshot(snapshot_id)) {
                self.selected_item = None;
            }
            self.has_unsaved_changes = true;
            self.last_export = None;
            self.refresh_validation();
        }
    }

    fn create_event_in_selected_bank(&mut self) {
        let event_name = self.new_event_name.trim().to_owned();
        if event_name.is_empty() {
            return;
        }

        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.clone() else {
            return;
        };
        let Some(first_asset) = project.assets.first() else {
            return;
        };

        let sampler_id = NodeId::new();
        let event = Event {
            id: sonara_model::EventId::new(),
            name: event_name.clone().into(),
            kind: EventKind::OneShot,
            root: EventContentRoot {
                root: NodeRef { id: sampler_id },
                nodes: vec![EventContentNode::Sampler(SamplerNode {
                    id: sampler_id,
                    asset_id: first_asset.id,
                })],
            },
            default_bus: None,
            spatial: SpatialMode::TwoD,
            default_parameters: Vec::new(),
            voice_limit: None,
            steal_policy: None,
        };
        let event_id = event.id;
        project.events.push(event);
        self.new_event_name.clear();
        self.add_event_to_selected_bank(event_id);
        self.selected_item = Some(SelectedItem::Event(event_id));
        self.status_message = self.tr(TextTemplate::CreatedEventInBank {
            event_name: event_name.clone(),
            bank_name: bank_name.clone(),
        });
        self.push_info_log(self.tr(TextTemplate::CreatedEventInBank {
            event_name,
            bank_name,
        }));
    }

    fn create_bus_in_selected_bank(&mut self) {
        let bus_name = self.new_bus_name.trim().to_owned();
        if bus_name.is_empty() {
            return;
        }

        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.clone() else {
            return;
        };

        let bus = Bus::new(bus_name.clone());
        let bus_id = bus.id;
        project.buses.push(bus);
        self.new_bus_name.clear();
        self.add_bus_to_selected_bank(bus_id);
        self.selected_item = Some(SelectedItem::Bus(bus_id));
        self.status_message = self.tr(TextTemplate::CreatedBusInBank {
            bus_name: bus_name.clone(),
            bank_name: bank_name.clone(),
        });
        self.push_info_log(self.tr(TextTemplate::CreatedBusInBank {
            bus_name,
            bank_name,
        }));
    }

    fn create_snapshot_in_selected_bank(&mut self) {
        let snapshot_name = self.new_snapshot_name.trim().to_owned();
        if snapshot_name.is_empty() {
            return;
        }

        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };
        let Some(bank_name) = self.selected_bank_name.clone() else {
            return;
        };

        let snapshot = Snapshot {
            id: sonara_model::SnapshotId::new(),
            name: snapshot_name.clone().into(),
            fade_in_seconds: 0.0,
            fade_out_seconds: 0.0,
            targets: Vec::new(),
        };
        let snapshot_id = snapshot.id;
        project.snapshots.push(snapshot);
        self.new_snapshot_name.clear();
        self.add_snapshot_to_selected_bank(snapshot_id);
        self.selected_item = Some(SelectedItem::Snapshot(snapshot_id));
        self.status_message = self.tr(TextTemplate::CreatedSnapshotInBank {
            snapshot_name: snapshot_name.clone(),
            bank_name: bank_name.clone(),
        });
        self.push_info_log(self.tr(TextTemplate::CreatedSnapshotInBank {
            snapshot_name,
            bank_name,
        }));
    }

    fn create_enum_parameter(&mut self) {
        let parameter_name = self.new_parameter_name.trim().to_owned();
        if parameter_name.is_empty() {
            return;
        }

        let variants = parse_variant_list(&self.new_parameter_variants);
        if variants.is_empty() {
            self.status_message = self.tx(TextKey::CreateParameterNeedsVariants).to_owned();
            return;
        }

        let Some(project) = self.loaded_project.as_mut() else {
            return;
        };

        let parameter = EnumParameter {
            id: sonara_model::ParameterId::new(),
            name: parameter_name.clone().into(),
            scope: ParameterScope::Global,
            default_value: variants[0].clone().into(),
            variants: variants.iter().cloned().map(Into::into).collect(),
        };
        let parameter_id = parameter.id;
        project.parameters.push(Parameter::Enum(parameter));
        self.new_parameter_name.clear();
        self.new_parameter_variants.clear();
        self.selected_item = Some(SelectedItem::Parameter(parameter_id));
        self.on_project_changed();
        self.status_message = self.tr(TextTemplate::CreatedParameter {
            parameter_name: parameter_name.clone(),
        });
        self.push_info_log(self.tr(TextTemplate::CreatedParameter { parameter_name }));
    }

    fn refresh_validation(&mut self) {
        let Some(project) = &self.loaded_project else {
            self.validation_report = ValidationReport::default();
            return;
        };

        let Some(bank) = self.selected_bank(project) else {
            self.validation_report = ValidationReport::default();
            return;
        };

        match compile_bank_definition(bank, project) {
            Ok(package) => {
                self.validation_report = ValidationReport::ready(
                    package.bank.manifest.assets.len(),
                    package.bank.manifest.resident_media.len(),
                    package.bank.manifest.streaming_media.len(),
                );
            }
            Err(error) => {
                self.validation_report = ValidationReport::blocked(vec![render_build_error(error)]);
            }
        }
    }

    fn on_project_changed(&mut self) {
        self.has_unsaved_changes = true;
        self.last_export = None;
        self.refresh_validation();
    }
}

impl ValidationReport {
    fn ready(
        asset_count: usize,
        resident_media_count: usize,
        streaming_media_count: usize,
    ) -> Self {
        Self {
            issues: Vec::new(),
            asset_count: Some(asset_count),
            resident_media_count: Some(resident_media_count),
            streaming_media_count: Some(streaming_media_count),
        }
    }

    fn blocked(issues: Vec<String>) -> Self {
        Self {
            issues,
            asset_count: None,
            resident_media_count: None,
            streaming_media_count: None,
        }
    }

    fn can_export(&self) -> bool {
        self.issues.is_empty()
    }
}

impl ExportReport {
    fn success(
        bank_name: String,
        output_path: String,
        event_count: usize,
        bus_count: usize,
        snapshot_count: usize,
        asset_count: usize,
        resident_media_count: usize,
        streaming_media_count: usize,
        file_size_bytes: Option<u64>,
    ) -> Self {
        Self {
            success: true,
            bank_name,
            output_path,
            event_count,
            bus_count,
            snapshot_count,
            asset_count,
            resident_media_count,
            streaming_media_count,
            file_size_bytes,
            error_message: None,
        }
    }

    fn failure(bank_name: String, output_path: String, error_message: String) -> Self {
        Self {
            success: false,
            bank_name,
            output_path,
            event_count: 0,
            bus_count: 0,
            snapshot_count: 0,
            asset_count: 0,
            resident_media_count: 0,
            streaming_media_count: 0,
            file_size_bytes: None,
            error_message: Some(error_message),
        }
    }
}

impl LogEntry {
    fn new(level: LogLevel, message: String) -> Self {
        Self {
            timestamp: current_timestamp_label(),
            level,
            message,
        }
    }
}

fn current_timestamp_label() -> String {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "time-error".to_owned();
    };

    format!("{}", duration.as_secs())
}

fn render_project_error(error: &ProjectFileError) -> String {
    error.to_string()
}

fn render_export_error(error: &ProjectExportBankError) -> String {
    error.to_string()
}

fn render_build_error(error: BuildError) -> String {
    match error {
        BuildError::EmptyEventTree => "事件内容树为空".to_owned(),
        BuildError::MissingRootNode => "事件根节点不存在".to_owned(),
        BuildError::DuplicateNodeId => "事件内容树存在重复节点 ID".to_owned(),
        BuildError::MissingChildNode => "节点引用了不存在的子节点".to_owned(),
        BuildError::EmptyContainer => "容器节点必须至少包含一个子节点".to_owned(),
        BuildError::MissingAudioAsset => "事件引用了不存在的音频资源".to_owned(),
        BuildError::MissingEventDefinition => "Bank引用了不存在的事件".to_owned(),
        BuildError::MissingBusDefinition => "Bank引用了不存在的总线".to_owned(),
        BuildError::MissingSnapshotDefinition => "Bank引用了不存在的快照".to_owned(),
        BuildError::MissingParameterDefinition => "事件 switch 引用了不存在的参数".to_owned(),
        BuildError::SwitchParameterNotEnum => "事件 switch 必须绑定枚举参数".to_owned(),
        BuildError::UnknownSwitchVariant => "事件 switch 使用了参数中不存在的枚举值".to_owned(),
    }
}

fn format_event_kind(kind: EventKind) -> &'static str {
    match kind {
        EventKind::OneShot => "OneShot",
        EventKind::Persistent => "Persistent",
    }
}

fn format_event_kind_display(locale: EditorLocale, kind: EventKind) -> &'static str {
    match locale {
        EditorLocale::ZhCn => match kind {
            EventKind::OneShot => "单次",
            EventKind::Persistent => "常驻",
        },
        EditorLocale::EnUs => format_event_kind(kind),
    }
}

fn format_spatial_mode_display(locale: EditorLocale, spatial: SpatialMode) -> &'static str {
    match locale {
        EditorLocale::ZhCn => match spatial {
            SpatialMode::None => "无",
            SpatialMode::TwoD => "2D",
            SpatialMode::ThreeD => "3D",
        },
        EditorLocale::EnUs => match spatial {
            SpatialMode::None => "None",
            SpatialMode::TwoD => "2D",
            SpatialMode::ThreeD => "3D",
        },
    }
}

fn format_parameter_scope_display(locale: EditorLocale, scope: ParameterScope) -> &'static str {
    match locale {
        EditorLocale::ZhCn => match scope {
            ParameterScope::Global => "全局",
            ParameterScope::Emitter => "发声体",
            ParameterScope::EventInstance => "事件实例",
        },
        EditorLocale::EnUs => match scope {
            ParameterScope::Global => "Global",
            ParameterScope::Emitter => "Emitter",
            ParameterScope::EventInstance => "Event Instance",
        },
    }
}
