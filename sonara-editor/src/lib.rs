//! Sonara 编辑器最小壳子。
//!
//! 当前阶段只打通 authoring 项目读取和 compiled bank 导出流程。

mod i18n;

use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use eframe::egui::{self, Align, Color32, Layout, RichText, TextEdit};
use egui_chinese_font::setup_chinese_fonts;
use i18n::{EditorLocale, TextKey, TextTemplate, template, text};
use sonara_build::{ProjectExportBankError, collect_event_asset_ids, compile_project_bank_to_file};
use sonara_model::{
    AuthoringProject, BankDefinition, Event, EventKind, ProjectFileError, SpatialMode,
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
        self.state.draw_top_bar(ctx);
        self.state.draw_left_panel(ctx);
        self.state.draw_right_panel(ctx);
        self.state.draw_bottom_panel(ctx);
        self.state.draw_center_panel(ctx);
    }
}

/// 编辑器运行时状态。
///
/// 这一层只维护 UI 所需的瞬时状态, 不把 authoring 模型和 UI 容器硬耦合。
#[derive(Debug, Default)]
pub struct EditorState {
    pub locale: EditorLocale,
    pub project_path: String,
    pub export_path: String,
    pub loaded_project: Option<AuthoringProject>,
    pub selected_bank_name: Option<String>,
    pub last_export: Option<ExportReport>,
    pub status_message: String,
    pub logs: Vec<LogEntry>,
}

impl EditorState {
    fn tx(&self, key: TextKey) -> &'static str {
        text(self.locale, key)
    }

    fn tr(&self, template_value: TextTemplate) -> String {
        template(self.locale, template_value)
    }

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("project_toolbar")
            .resizable(false)
            .show(ctx, |ui| {
                let project_path_hint = self.tx(TextKey::ProjectPathHint);
                let load_project_label = self.tx(TextKey::LoadProject);
                let language_label = self.tx(TextKey::Language);

                ui.horizontal(|ui| {
                    ui.label(self.tx(TextKey::ProjectPath));
                    ui.add(
                        TextEdit::singleline(&mut self.project_path)
                            .desired_width(f32::INFINITY)
                            .hint_text(project_path_hint),
                    );

                    if ui.button(load_project_label).clicked() {
                        self.load_project();
                    }

                    ui.separator();
                    ui.label(language_label);
                    egui::ComboBox::from_id_salt("editor_locale")
                        .selected_text(self.locale.display_name())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.locale,
                                EditorLocale::ZhCn,
                                EditorLocale::ZhCn.display_name(),
                            );
                            ui.selectable_value(
                                &mut self.locale,
                                EditorLocale::EnUs,
                                EditorLocale::EnUs.display_name(),
                            );
                        });
                });

                if !self.status_message.is_empty() {
                    ui.label(RichText::new(&self.status_message).color(Color32::LIGHT_BLUE));
                }
            });
    }

    fn draw_left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("project_panel")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading(self.tx(TextKey::ProjectPanel));
                ui.separator();

                let Some(project) = &self.loaded_project else {
                    ui.label(self.tx(TextKey::NoProjectLoaded));
                    return;
                };

                ui.label(format!("{}: {}", self.tx(TextKey::Name), project.name));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::Assets),
                    project.assets.len()
                ));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::Events),
                    project.events.len()
                ));
                ui.label(format!(
                    "{}: {}",
                    self.tx(TextKey::Banks),
                    project.banks.len()
                ));

                ui.separator();
                ui.label(RichText::new(self.tx(TextKey::BankList)).strong());

                let bank_names: Vec<String> = project
                    .banks
                    .iter()
                    .map(|bank| bank.name.to_string())
                    .collect();
                let mut clicked_bank_name = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for bank_name in &bank_names {
                        let selected =
                            self.selected_bank_name.as_deref() == Some(bank_name.as_str());
                        if ui.selectable_label(selected, bank_name.as_str()).clicked() {
                            clicked_bank_name = Some(bank_name.clone());
                        }
                    }
                });

                if let Some(bank_name) = clicked_bank_name {
                    self.select_bank(&bank_name);
                }
            });
    }

    fn draw_center_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(self.tx(TextKey::BankExport));
            ui.separator();

            let Some(project) = &self.loaded_project else {
                ui.label(self.tx(TextKey::LoadProjectFirst));
                return;
            };

            let Some(selected_bank_name) = self.selected_bank_name.clone() else {
                ui.label(self.tx(TextKey::SelectBankFirst));
                return;
            };

            let Some(bank) = project.bank_named(&selected_bank_name) else {
                ui.colored_label(Color32::RED, self.tx(TextKey::SelectedBankMissing));
                return;
            };

            ui.label(format!("{}: {}", self.tx(TextKey::CurrentBank), bank.name));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::EventCount),
                bank.events.len()
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::BusCount),
                bank.buses.len()
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::SnapshotCount),
                bank.snapshots.len()
            ));

            ui.add_space(8.0);
            let output_path_hint = self.tx(TextKey::OutputPathHint);
            ui.label(self.tx(TextKey::OutputPath));
            ui.add(
                TextEdit::singleline(&mut self.export_path)
                    .desired_width(f32::INFINITY)
                    .hint_text(output_path_hint),
            );

            ui.add_space(8.0);
            let mut should_export = false;
            let mut should_reset_export_path = false;
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                if ui.button(self.tx(TextKey::ExportCompiledBank)).clicked() {
                    should_export = true;
                }

                if ui
                    .button(self.tx(TextKey::ResetDefaultExportPath))
                    .clicked()
                {
                    should_reset_export_path = true;
                }
            });

            if should_export {
                self.export_selected_bank();
            }

            if should_reset_export_path {
                self.export_path = self.suggest_export_path(&selected_bank_name);
            }

            ui.add_space(12.0);
            ui.group(|ui| {
                ui.label(RichText::new(self.tx(TextKey::ExportGuide)).strong());
                ui.label(self.tx(TextKey::ExportGuideLine1));
                ui.label(self.tx(TextKey::ExportGuideLine2));
            });

            ui.add_space(12.0);
            self.draw_export_report(ui);
        });
    }

    fn draw_right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector_panel")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading(self.tx(TextKey::Inspector));
                ui.separator();

                let Some(project) = &self.loaded_project else {
                    ui.label(self.tx(TextKey::NoProjectLoadedShort));
                    return;
                };

                let Some(bank) = self.selected_bank(project) else {
                    ui.label(self.tx(TextKey::NoBankSelected));
                    return;
                };

                self.draw_event_list(ui, project, bank);
                ui.separator();
                self.draw_bus_list(ui, project, bank);
                ui.separator();
                self.draw_snapshot_list(ui, project, bank);
            });
    }

    fn draw_bottom_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .default_height(160.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(self.tx(TextKey::Log));
                    if ui.button(self.tx(TextKey::Clear)).clicked() {
                        self.logs.clear();
                    }
                });
                ui.separator();

                if self.logs.is_empty() {
                    ui.label(self.tx(TextKey::NoLogs));
                    return;
                }

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for entry in &self.logs {
                            let color = match entry.level {
                                LogLevel::Info => Color32::LIGHT_GREEN,
                                LogLevel::Error => Color32::LIGHT_RED,
                            };
                            ui.label(
                                RichText::new(format!("[{}] {}", entry.timestamp, entry.message))
                                    .color(color),
                            );
                        }
                    });
            });
    }

    fn draw_event_list(
        &self,
        ui: &mut egui::Ui,
        project: &AuthoringProject,
        bank: &BankDefinition,
    ) {
        ui.label(RichText::new(self.tx(TextKey::Events)).strong());
        if bank.events.is_empty() {
            ui.label(self.tx(TextKey::NoEvents));
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("event_list")
            .max_height(260.0)
            .show(ui, |ui| {
                for event_id in &bank.events {
                    if let Some(event) = project.events.iter().find(|event| event.id == *event_id) {
                        let asset_count = collect_event_asset_ids(event).len();
                        let summary = format!(
                            "{} | {} | {}",
                            event.name,
                            format_event_kind(event.kind),
                            format_spatial_mode(event.spatial)
                        );
                        ui.collapsing(summary, |ui| {
                            ui.label(format!("ID: {}", format_event_id(event)));
                            ui.label(format!(
                                "{}: {}",
                                self.tx(TextKey::Kind),
                                format_event_kind(event.kind)
                            ));
                            ui.label(format!(
                                "{}: {}",
                                self.tx(TextKey::Spatial),
                                format_spatial_mode(event.spatial)
                            ));
                            ui.label(format!(
                                "{}: {}",
                                self.tx(TextKey::NodeCount),
                                event.root.nodes.len()
                            ));
                            ui.label(format!(
                                "{}: {}",
                                self.tx(TextKey::ResolvedAssetCount),
                                asset_count
                            ));
                        });
                    } else {
                        ui.colored_label(
                            Color32::YELLOW,
                            format!("{}: {}", self.tx(TextKey::MissingEvent), event_id.0),
                        );
                    }
                }
            });
    }

    fn draw_bus_list(&self, ui: &mut egui::Ui, project: &AuthoringProject, bank: &BankDefinition) {
        ui.label(RichText::new(self.tx(TextKey::Buses)).strong());
        if bank.buses.is_empty() {
            ui.label(self.tx(TextKey::NoBuses));
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("bus_list")
            .max_height(140.0)
            .show(ui, |ui| {
                for bus_id in &bank.buses {
                    if let Some(bus) = project.buses.iter().find(|bus| bus.id == *bus_id) {
                        ui.label(format!("{} | {}", bus.name, bus.id.0));
                    } else {
                        ui.colored_label(
                            Color32::YELLOW,
                            format!("{}: {}", self.tx(TextKey::MissingBus), bus_id.0),
                        );
                    }
                }
            });
    }

    fn draw_snapshot_list(
        &self,
        ui: &mut egui::Ui,
        project: &AuthoringProject,
        bank: &BankDefinition,
    ) {
        ui.label(RichText::new(self.tx(TextKey::Snapshots)).strong());
        if bank.snapshots.is_empty() {
            ui.label(self.tx(TextKey::NoSnapshots));
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("snapshot_list")
            .max_height(140.0)
            .show(ui, |ui| {
                for snapshot_id in &bank.snapshots {
                    if let Some(snapshot) = project
                        .snapshots
                        .iter()
                        .find(|snapshot| snapshot.id == *snapshot_id)
                    {
                        ui.label(format!("{} | {}", snapshot.name, snapshot.id.0));
                    } else {
                        ui.colored_label(
                            Color32::YELLOW,
                            format!("{}: {}", self.tx(TextKey::MissingSnapshot), snapshot_id.0),
                        );
                    }
                }
            });
    }

    /// 加载当前路径指向的 project 文件。
    pub fn load_project(&mut self) {
        match AuthoringProject::read_json_file(&self.project_path) {
            Ok(project) => {
                self.loaded_project = Some(project);
                self.last_export = None;
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
        self.export_path = self.suggest_export_path(bank_name);
        self.status_message = self.tr(TextTemplate::SelectBank {
            bank_name: bank_name.to_owned(),
        });
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

    fn draw_export_report(&self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label(RichText::new(self.tx(TextKey::LastExport)).strong());

            let Some(report) = &self.last_export else {
                ui.label(self.tx(TextKey::NoExportYet));
                return;
            };

            let status_label = if report.success {
                self.tx(TextKey::LastExportSuccess)
            } else {
                self.tx(TextKey::LastExportFailure)
            };
            let status_color = if report.success {
                Color32::LIGHT_GREEN
            } else {
                Color32::LIGHT_RED
            };

            ui.label(RichText::new(status_label).color(status_color));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::CurrentBank),
                report.bank_name
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::OutputPath),
                report.output_path
            ));

            if let Some(error_message) = &report.error_message {
                ui.colored_label(Color32::LIGHT_RED, error_message);
                return;
            }

            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::EventCount),
                report.event_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::BusCount),
                report.bus_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::SnapshotCount),
                report.snapshot_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::AssetCount),
                report.asset_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::ResidentMediaCount),
                report.resident_media_count
            ));
            ui.label(format!(
                "{}: {}",
                self.tx(TextKey::StreamingMediaCount),
                report.streaming_media_count
            ));

            if let Some(file_size_bytes) = report.file_size_bytes {
                ui.label(format!(
                    "{}: {} B",
                    self.tx(TextKey::FileSizeBytes),
                    file_size_bytes
                ));
            }
        });
    }
}

fn format_event_id(event: &Event) -> String {
    event.id.0.to_string()
}

fn render_project_error(error: &ProjectFileError) -> String {
    error.to_string()
}

fn render_export_error(error: &ProjectExportBankError) -> String {
    error.to_string()
}

fn format_event_kind(kind: EventKind) -> &'static str {
    match kind {
        EventKind::OneShot => "OneShot",
        EventKind::Persistent => "Persistent",
    }
}

fn format_spatial_mode(spatial: SpatialMode) -> &'static str {
    match spatial {
        SpatialMode::None => "None",
        SpatialMode::TwoD => "TwoD",
        SpatialMode::ThreeD => "ThreeD",
    }
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
