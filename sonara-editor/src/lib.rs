//! Sonara 编辑器最小壳子。
//!
//! 当前阶段只打通 authoring 项目读取和 compiled bank 导出流程。

use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use eframe::egui::{self, Align, Color32, Layout, RichText, TextEdit};
use sonara_build::{ProjectExportBankError, compile_project_bank_to_file};
use sonara_model::{AuthoringProject, BankDefinition, Event, ProjectFileError};

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
        Box::new(|_cc| Ok(Box::new(EditorApp::new()))),
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
    pub project_path: String,
    pub export_path: String,
    pub loaded_project: Option<AuthoringProject>,
    pub selected_bank_name: Option<String>,
    pub status_message: String,
    pub logs: Vec<LogEntry>,
}

impl EditorState {
    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("project_toolbar")
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Project 路径");
                    ui.add(
                        TextEdit::singleline(&mut self.project_path)
                            .desired_width(f32::INFINITY)
                            .hint_text("输入 project.json 路径"),
                    );

                    if ui.button("加载 project").clicked() {
                        self.load_project();
                    }
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
                ui.heading("Project");
                ui.separator();

                let Some(project) = &self.loaded_project else {
                    ui.label("尚未加载 project.json");
                    return;
                };

                ui.label(format!("名称: {}", project.name));
                ui.label(format!("Assets: {}", project.assets.len()));
                ui.label(format!("Events: {}", project.events.len()));
                ui.label(format!("Banks: {}", project.banks.len()));

                ui.separator();
                ui.label(RichText::new("Bank 列表").strong());

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
            ui.heading("Bank Export");
            ui.separator();

            let Some(project) = &self.loaded_project else {
                ui.label("先加载一个 authoring project。");
                return;
            };

            let Some(selected_bank_name) = self.selected_bank_name.clone() else {
                ui.label("先从左侧选择一个 bank。");
                return;
            };

            let Some(bank) = project.bank_named(&selected_bank_name) else {
                ui.colored_label(Color32::RED, "当前选中的 bank 已不存在。");
                return;
            };

            ui.label(format!("当前 bank: {}", bank.name));
            ui.label(format!("事件数量: {}", bank.events.len()));
            ui.label(format!("Bus 数量: {}", bank.buses.len()));
            ui.label(format!("Snapshot 数量: {}", bank.snapshots.len()));

            ui.add_space(8.0);
            ui.label("输出路径");
            ui.add(
                TextEdit::singleline(&mut self.export_path)
                    .desired_width(f32::INFINITY)
                    .hint_text("输入导出的 compiled bank JSON 路径"),
            );

            ui.add_space(8.0);
            let mut should_export = false;
            let mut should_reset_export_path = false;
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                if ui.button("导出 compiled bank").clicked() {
                    should_export = true;
                }

                if ui.button("重置默认输出路径").clicked() {
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
                ui.label(RichText::new("导出说明").strong());
                ui.label("编辑器读取的是 project.json。");
                ui.label("导出按钮会调用 sonara-build 生成 runtime 使用的 compiled bank JSON。");
            });
        });
    }

    fn draw_right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector_panel")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Inspector");
                ui.separator();

                let Some(project) = &self.loaded_project else {
                    ui.label("尚未加载 project。");
                    return;
                };

                let Some(bank) = self.selected_bank(project) else {
                    ui.label("尚未选择 bank。");
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
                    ui.heading("Log");
                    if ui.button("清空").clicked() {
                        self.logs.clear();
                    }
                });
                ui.separator();

                if self.logs.is_empty() {
                    ui.label("暂无日志。");
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
        ui.label(RichText::new("Events").strong());
        if bank.events.is_empty() {
            ui.label("无 events。");
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("event_list")
            .max_height(180.0)
            .show(ui, |ui| {
                for event_id in &bank.events {
                    if let Some(event) = project.events.iter().find(|event| event.id == *event_id) {
                        ui.label(format!("{} | {}", event.name, format_event_id(event)));
                    } else {
                        ui.colored_label(Color32::YELLOW, format!("缺失 event: {}", event_id.0));
                    }
                }
            });
    }

    fn draw_bus_list(&self, ui: &mut egui::Ui, project: &AuthoringProject, bank: &BankDefinition) {
        ui.label(RichText::new("Buses").strong());
        if bank.buses.is_empty() {
            ui.label("无 buses。");
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
                        ui.colored_label(Color32::YELLOW, format!("缺失 bus: {}", bus_id.0));
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
        ui.label(RichText::new("Snapshots").strong());
        if bank.snapshots.is_empty() {
            ui.label("无 snapshots。");
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
                            format!("缺失 snapshot: {}", snapshot_id.0),
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
                self.status_message = format!("已加载 project: {}", self.project_path);

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

                self.push_info_log(format!("加载成功: {}", self.project_path));
            }
            Err(error) => {
                self.loaded_project = None;
                self.selected_bank_name = None;
                self.status_message = format!("加载失败: {}", render_project_error(&error));
                self.push_error_log(format!(
                    "加载 project 失败, path={}, error={}",
                    self.project_path,
                    render_project_error(&error)
                ));
            }
        }
    }

    /// 选择当前项目里的一个 bank。
    pub fn select_bank(&mut self, bank_name: &str) {
        self.selected_bank_name = Some(bank_name.to_owned());
        self.export_path = self.suggest_export_path(bank_name);
        self.status_message = format!("已选择 bank: {bank_name}");
    }

    /// 导出当前选中的 bank。
    pub fn export_selected_bank(&mut self) {
        let Some(project) = &self.loaded_project else {
            self.status_message = "导出失败: 尚未加载 project".to_owned();
            self.push_error_log("导出失败, 尚未加载 project".to_owned());
            return;
        };

        let Some(bank_name) = self.selected_bank_name.clone() else {
            self.status_message = "导出失败: 尚未选择 bank".to_owned();
            self.push_error_log("导出失败, 尚未选择 bank".to_owned());
            return;
        };

        if self.export_path.trim().is_empty() {
            self.status_message = "导出失败: 输出路径不能为空".to_owned();
            self.push_error_log("导出失败, 输出路径为空".to_owned());
            return;
        }

        match compile_project_bank_to_file(project, &bank_name, &self.export_path) {
            Ok(package) => {
                self.status_message =
                    format!("导出成功: {} -> {}", package.bank.name, self.export_path);
                self.push_info_log(format!(
                    "导出成功, bank={}, events={}, output={}",
                    package.bank.name,
                    package.events.len(),
                    self.export_path
                ));
            }
            Err(error) => {
                self.status_message = format!("导出失败: {}", render_export_error(&error));
                self.push_error_log(format!(
                    "导出失败, bank={}, output={}, error={}",
                    bank_name,
                    self.export_path,
                    render_export_error(&error)
                ));
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
