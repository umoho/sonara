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
    AssetCount,
    ResidentMediaCount,
    StreamingMediaCount,
    OutputPath,
    OutputPathHint,
    ExportCompiledBank,
    ResetDefaultExportPath,
    ExportGuide,
    ExportGuideLine1,
    ExportGuideLine2,
    LastExport,
    NoExportYet,
    LastExportSuccess,
    LastExportFailure,
    FileSizeBytes,
    Inspector,
    NoProjectLoadedShort,
    NoBankSelected,
    Buses,
    Snapshots,
    Kind,
    Spatial,
    NodeCount,
    ResolvedAssetCount,
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
        TextKey::ProjectPath => "项目路径",
        TextKey::ProjectPathHint => "输入项目文件路径",
        TextKey::LoadProject => "加载项目",
        TextKey::Language => "语言",
        TextKey::ProjectPanel => "项目",
        TextKey::NoProjectLoaded => "尚未加载项目文件",
        TextKey::Name => "名称",
        TextKey::Assets => "资源",
        TextKey::Events => "事件",
        TextKey::Banks => "音频包",
        TextKey::BankList => "音频包列表",
        TextKey::BankExport => "音频包导出",
        TextKey::LoadProjectFirst => "请先加载一个项目文件",
        TextKey::SelectBankFirst => "请先从左侧选择一个音频包",
        TextKey::SelectedBankMissing => "当前选中的音频包已不存在",
        TextKey::CurrentBank => "当前音频包",
        TextKey::EventCount => "事件数量",
        TextKey::BusCount => "总线数量",
        TextKey::SnapshotCount => "快照数量",
        TextKey::AssetCount => "资源数量",
        TextKey::ResidentMediaCount => "常驻资源数量",
        TextKey::StreamingMediaCount => "流式资源数量",
        TextKey::OutputPath => "输出路径",
        TextKey::OutputPathHint => "输入导出的编译结果路径",
        TextKey::ExportCompiledBank => "导出编译结果",
        TextKey::ResetDefaultExportPath => "重置默认输出路径",
        TextKey::ExportGuide => "导出说明",
        TextKey::ExportGuideLine1 => "编辑器读取的是项目文件",
        TextKey::ExportGuideLine2 => "导出按钮会调用 sonara-build 生成运行时使用的编译结果 JSON",
        TextKey::LastExport => "最近一次导出",
        TextKey::NoExportYet => "尚未执行导出",
        TextKey::LastExportSuccess => "成功",
        TextKey::LastExportFailure => "失败",
        TextKey::FileSizeBytes => "文件大小",
        TextKey::Inspector => "检查器",
        TextKey::NoProjectLoadedShort => "尚未加载项目",
        TextKey::NoBankSelected => "尚未选择音频包",
        TextKey::Buses => "总线",
        TextKey::Snapshots => "快照",
        TextKey::Kind => "类型",
        TextKey::Spatial => "空间化",
        TextKey::NodeCount => "节点数量",
        TextKey::ResolvedAssetCount => "引用资源数量",
        TextKey::NoEvents => "无事件",
        TextKey::NoBuses => "无总线",
        TextKey::NoSnapshots => "无快照",
        TextKey::MissingEvent => "缺失事件",
        TextKey::MissingBus => "缺失总线",
        TextKey::MissingSnapshot => "缺失快照",
        TextKey::Log => "日志",
        TextKey::Clear => "清空",
        TextKey::NoLogs => "暂无日志",
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
        TextKey::AssetCount => "Asset Count",
        TextKey::ResidentMediaCount => "Resident Media Count",
        TextKey::StreamingMediaCount => "Streaming Media Count",
        TextKey::OutputPath => "Output Path",
        TextKey::OutputPathHint => "Enter the output path for the compiled bank JSON",
        TextKey::ExportCompiledBank => "Export Compiled Bank",
        TextKey::ResetDefaultExportPath => "Reset Default Output Path",
        TextKey::ExportGuide => "Export Notes",
        TextKey::ExportGuideLine1 => "The editor reads project.json.",
        TextKey::ExportGuideLine2 => {
            "The export action calls sonara-build to generate the compiled bank JSON for runtime."
        }
        TextKey::LastExport => "Last Export",
        TextKey::NoExportYet => "No export has been run yet.",
        TextKey::LastExportSuccess => "Success",
        TextKey::LastExportFailure => "Failure",
        TextKey::FileSizeBytes => "File Size",
        TextKey::Inspector => "Inspector",
        TextKey::NoProjectLoadedShort => "No project loaded.",
        TextKey::NoBankSelected => "No bank selected.",
        TextKey::Buses => "Buses",
        TextKey::Snapshots => "Snapshots",
        TextKey::Kind => "Kind",
        TextKey::Spatial => "Spatial",
        TextKey::NodeCount => "Node Count",
        TextKey::ResolvedAssetCount => "Referenced Asset Count",
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
        TextTemplate::ProjectLoaded { path } => format!("已加载项目: {path}"),
        TextTemplate::LoadFailed { error } => format!("加载失败: {error}"),
        TextTemplate::LoadSucceeded { path } => format!("项目加载成功: {path}"),
        TextTemplate::SelectBank { bank_name } => format!("已选择音频包: {bank_name}"),
        TextTemplate::ExportFailedNoProject => "导出失败: 尚未加载项目".to_owned(),
        TextTemplate::ExportFailedNoBank => "导出失败: 尚未选择音频包".to_owned(),
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
        } => format!("导出成功, 音频包={bank_name}, 事件数={event_count}, 输出={output_path}"),
        TextTemplate::ExportFailedLog {
            bank_name,
            output_path,
            error,
        } => format!("导出失败, 音频包={bank_name}, 输出={output_path}, 错误={error}"),
        TextTemplate::LoadFailedLog { path, error } => {
            format!("加载项目失败, 路径={path}, 错误={error}")
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
