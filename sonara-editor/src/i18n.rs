//! 编辑器 UI 的最小 i18n 支撑。
//!
//! 当前只覆盖 editor 自己的界面文案, 不向 model/build/runtime 扩散。

/// 编辑器支持的 UI 语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorLocale {
    /// 简体中文。
    #[default]
    ZhCn,
    /// 英文。
    EnUs,
}

impl EditorLocale {
    /// 返回语言选择器里展示的标签。
    pub fn display_name(self) -> &'static str {
        match self {
            Self::ZhCn => "简体中文",
            Self::EnUs => "English",
        }
    }
}

/// UI 静态文案 key。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKey {
    ProjectPath,
    ProjectPathHint,
    LoadProject,
    Language,
    ProjectPanel,
    NoProjectLoaded,
    Name,
    Assets,
    Events,
    Banks,
    BankList,
    BankExport,
    LoadProjectFirst,
    SelectBankFirst,
    SelectedBankMissing,
    CurrentBank,
    EventCount,
    BusCount,
    SnapshotCount,
    OutputPath,
    OutputPathHint,
    ExportCompiledBank,
    ResetDefaultExportPath,
    ExportGuide,
    ExportGuideLine1,
    ExportGuideLine2,
    Inspector,
    NoProjectLoadedShort,
    NoBankSelected,
    Buses,
    Snapshots,
    NoEvents,
    NoBuses,
    NoSnapshots,
    MissingEvent,
    MissingBus,
    MissingSnapshot,
    Log,
    Clear,
    NoLogs,
    FontLoaded,
}

/// 翻译带参数的文案。
#[derive(Debug, Clone)]
pub enum TextTemplate {
    FontLoadFailed {
        error: String,
    },
    ProjectLoaded {
        path: String,
    },
    LoadFailed {
        error: String,
    },
    LoadSucceeded {
        path: String,
    },
    SelectBank {
        bank_name: String,
    },
    ExportFailedNoProject,
    ExportFailedNoBank,
    ExportFailedEmptyOutputPath,
    ExportSucceeded {
        bank_name: String,
        output_path: String,
    },
    ExportFailed {
        error: String,
    },
    ExportSucceededLog {
        bank_name: String,
        event_count: usize,
        output_path: String,
    },
    ExportFailedLog {
        bank_name: String,
        output_path: String,
        error: String,
    },
    LoadFailedLog {
        path: String,
        error: String,
    },
}

/// 返回静态文案。
pub fn text(locale: EditorLocale, key: TextKey) -> &'static str {
    match locale {
        EditorLocale::ZhCn => text_zh_cn(key),
        EditorLocale::EnUs => text_en_us(key),
    }
}

/// 返回带参数的文案。
pub fn template(locale: EditorLocale, template: TextTemplate) -> String {
    match locale {
        EditorLocale::ZhCn => template_zh_cn(template),
        EditorLocale::EnUs => template_en_us(template),
    }
}

fn text_zh_cn(key: TextKey) -> &'static str {
    match key {
        TextKey::ProjectPath => "Project 路径",
        TextKey::ProjectPathHint => "输入 project.json 路径",
        TextKey::LoadProject => "加载 project",
        TextKey::Language => "语言",
        TextKey::ProjectPanel => "Project",
        TextKey::NoProjectLoaded => "尚未加载 project.json",
        TextKey::Name => "名称",
        TextKey::Assets => "Assets",
        TextKey::Events => "Events",
        TextKey::Banks => "Banks",
        TextKey::BankList => "Bank 列表",
        TextKey::BankExport => "Bank Export",
        TextKey::LoadProjectFirst => "先加载一个 authoring project。",
        TextKey::SelectBankFirst => "先从左侧选择一个 bank。",
        TextKey::SelectedBankMissing => "当前选中的 bank 已不存在。",
        TextKey::CurrentBank => "当前 bank",
        TextKey::EventCount => "事件数量",
        TextKey::BusCount => "Bus 数量",
        TextKey::SnapshotCount => "Snapshot 数量",
        TextKey::OutputPath => "输出路径",
        TextKey::OutputPathHint => "输入导出的 compiled bank JSON 路径",
        TextKey::ExportCompiledBank => "导出 compiled bank",
        TextKey::ResetDefaultExportPath => "重置默认输出路径",
        TextKey::ExportGuide => "导出说明",
        TextKey::ExportGuideLine1 => "编辑器读取的是 project.json。",
        TextKey::ExportGuideLine2 => {
            "导出按钮会调用 sonara-build 生成 runtime 使用的 compiled bank JSON。"
        }
        TextKey::Inspector => "Inspector",
        TextKey::NoProjectLoadedShort => "尚未加载 project。",
        TextKey::NoBankSelected => "尚未选择 bank。",
        TextKey::Buses => "Buses",
        TextKey::Snapshots => "Snapshots",
        TextKey::NoEvents => "无 events。",
        TextKey::NoBuses => "无 buses。",
        TextKey::NoSnapshots => "无 snapshots。",
        TextKey::MissingEvent => "缺失 event",
        TextKey::MissingBus => "缺失 bus",
        TextKey::MissingSnapshot => "缺失 snapshot",
        TextKey::Log => "Log",
        TextKey::Clear => "清空",
        TextKey::NoLogs => "暂无日志。",
        TextKey::FontLoaded => "中文字体加载成功",
    }
}

fn text_en_us(key: TextKey) -> &'static str {
    match key {
        TextKey::ProjectPath => "Project Path",
        TextKey::ProjectPathHint => "Enter the path to project.json",
        TextKey::LoadProject => "Load Project",
        TextKey::Language => "Language",
        TextKey::ProjectPanel => "Project",
        TextKey::NoProjectLoaded => "No project.json loaded.",
        TextKey::Name => "Name",
        TextKey::Assets => "Assets",
        TextKey::Events => "Events",
        TextKey::Banks => "Banks",
        TextKey::BankList => "Bank List",
        TextKey::BankExport => "Bank Export",
        TextKey::LoadProjectFirst => "Load an authoring project first.",
        TextKey::SelectBankFirst => "Select a bank from the left panel first.",
        TextKey::SelectedBankMissing => "The selected bank no longer exists.",
        TextKey::CurrentBank => "Current Bank",
        TextKey::EventCount => "Event Count",
        TextKey::BusCount => "Bus Count",
        TextKey::SnapshotCount => "Snapshot Count",
        TextKey::OutputPath => "Output Path",
        TextKey::OutputPathHint => "Enter the output path for the compiled bank JSON",
        TextKey::ExportCompiledBank => "Export Compiled Bank",
        TextKey::ResetDefaultExportPath => "Reset Default Output Path",
        TextKey::ExportGuide => "Export Notes",
        TextKey::ExportGuideLine1 => "The editor reads project.json.",
        TextKey::ExportGuideLine2 => {
            "The export action calls sonara-build to generate the compiled bank JSON for runtime."
        }
        TextKey::Inspector => "Inspector",
        TextKey::NoProjectLoadedShort => "No project loaded.",
        TextKey::NoBankSelected => "No bank selected.",
        TextKey::Buses => "Buses",
        TextKey::Snapshots => "Snapshots",
        TextKey::NoEvents => "No events.",
        TextKey::NoBuses => "No buses.",
        TextKey::NoSnapshots => "No snapshots.",
        TextKey::MissingEvent => "Missing event",
        TextKey::MissingBus => "Missing bus",
        TextKey::MissingSnapshot => "Missing snapshot",
        TextKey::Log => "Log",
        TextKey::Clear => "Clear",
        TextKey::NoLogs => "No logs yet.",
        TextKey::FontLoaded => "Chinese font loaded successfully",
    }
}

fn template_zh_cn(template: TextTemplate) -> String {
    match template {
        TextTemplate::FontLoadFailed { error } => format!("中文字体加载失败: {error}"),
        TextTemplate::ProjectLoaded { path } => format!("已加载 project: {path}"),
        TextTemplate::LoadFailed { error } => format!("加载失败: {error}"),
        TextTemplate::LoadSucceeded { path } => format!("加载成功: {path}"),
        TextTemplate::SelectBank { bank_name } => format!("已选择 bank: {bank_name}"),
        TextTemplate::ExportFailedNoProject => "导出失败: 尚未加载 project".to_owned(),
        TextTemplate::ExportFailedNoBank => "导出失败: 尚未选择 bank".to_owned(),
        TextTemplate::ExportFailedEmptyOutputPath => "导出失败: 输出路径不能为空".to_owned(),
        TextTemplate::ExportSucceeded {
            bank_name,
            output_path,
        } => format!("导出成功: {bank_name} -> {output_path}"),
        TextTemplate::ExportFailed { error } => format!("导出失败: {error}"),
        TextTemplate::ExportSucceededLog {
            bank_name,
            event_count,
            output_path,
        } => format!("导出成功, bank={bank_name}, events={event_count}, output={output_path}"),
        TextTemplate::ExportFailedLog {
            bank_name,
            output_path,
            error,
        } => format!("导出失败, bank={bank_name}, output={output_path}, error={error}"),
        TextTemplate::LoadFailedLog { path, error } => {
            format!("加载 project 失败, path={path}, error={error}")
        }
    }
}

fn template_en_us(template: TextTemplate) -> String {
    match template {
        TextTemplate::FontLoadFailed { error } => format!("Failed to load Chinese font: {error}"),
        TextTemplate::ProjectLoaded { path } => format!("Loaded project: {path}"),
        TextTemplate::LoadFailed { error } => format!("Load failed: {error}"),
        TextTemplate::LoadSucceeded { path } => format!("Project loaded: {path}"),
        TextTemplate::SelectBank { bank_name } => format!("Selected bank: {bank_name}"),
        TextTemplate::ExportFailedNoProject => "Export failed: no project loaded".to_owned(),
        TextTemplate::ExportFailedNoBank => "Export failed: no bank selected".to_owned(),
        TextTemplate::ExportFailedEmptyOutputPath => {
            "Export failed: output path cannot be empty".to_owned()
        }
        TextTemplate::ExportSucceeded {
            bank_name,
            output_path,
        } => format!("Export succeeded: {bank_name} -> {output_path}"),
        TextTemplate::ExportFailed { error } => format!("Export failed: {error}"),
        TextTemplate::ExportSucceededLog {
            bank_name,
            event_count,
            output_path,
        } => format!(
            "Export succeeded, bank={bank_name}, events={event_count}, output={output_path}"
        ),
        TextTemplate::ExportFailedLog {
            bank_name,
            output_path,
            error,
        } => format!("Export failed, bank={bank_name}, output={output_path}, error={error}"),
        TextTemplate::LoadFailedLog { path, error } => {
            format!("Failed to load project, path={path}, error={error}")
        }
    }
}
