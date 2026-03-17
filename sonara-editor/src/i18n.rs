// SPDX-License-Identifier: MPL-2.0

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
    OpenProject,
    ProjectPath,
    ProjectPathHint,
    LoadProject,
    RecentProjects,
    Language,
    ProjectPanel,
    NoProjectLoaded,
    Name,
    Assets,
    Events,
    Banks,
    BankList,
    QuickActions,
    CreateObjects,
    NewAssetPath,
    NewParameterName,
    NewParameterVariants,
    CreateAsset,
    CreateEvent,
    CreateBus,
    CreateSnapshot,
    CreateEnumParameter,
    CreateEventNeedsAsset,
    CreateParameterNeedsVariants,
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
    SaveProject,
    UnsavedChanges,
    Validation,
    ValidationReady,
    ValidationBlocked,
    NoValidationIssues,
    ValidationIssueCount,
    LastExport,
    NoExportYet,
    LastExportSuccess,
    LastExportFailure,
    FileSizeBytes,
    WelcomeBack,
    WelcomeTitle,
    WelcomeSubtitle,
    ContinueProject,
    NoRecentProjects,
    Inspector,
    NoProjectLoadedShort,
    NoBankSelected,
    Buses,
    Snapshots,
    Parameters,
    Kind,
    Spatial,
    ParameterScope,
    ParameterVariants,
    DefaultValue,
    VariantCount,
    NodeCount,
    ResolvedAssetCount,
    BankContentsEditor,
    NoSelection,
    CurrentBankEvents,
    AvailableEvents,
    CurrentBankBuses,
    AvailableBuses,
    CurrentBankSnapshots,
    AvailableSnapshots,
    AddToBank,
    RemoveFromBank,
    DeleteFromProject,
    NoAvailableEvents,
    NoAvailableBuses,
    NoAvailableSnapshots,
    DefaultVolume,
    FadeInSeconds,
    FadeOutSeconds,
    EventContent,
    ContentMode,
    SingleAsset,
    StateSwitch,
    SwitchParameter,
    SwitchVariants,
    DefaultCase,
    ConvertToSingleAsset,
    ConvertToStateSwitch,
    ProjectParameters,
    ProjectAssets,
    NoParameters,
    NoAssetsInProject,
    NoEnumParameters,
    EnumParameterHint,
    AssetImportHint,
    UnsupportedEventContent,
    UnsupportedParameterType,
    NoEvents,
    NoBuses,
    NoSnapshots,
    Log,
    Clear,
    Confirm,
    Cancel,
    NoLogs,
    FontLoaded,
    Ready,
    SavedProjectStatus,
    CurrentProject,
    ToolStrip,
    PreviewUnavailable,
    MenuFile,
    MenuView,
    MenuWindow,
    MenuHelp,
    ToggleProjectPanel,
    ToggleInspectorPanel,
    ToggleToolStrip,
    ToggleStatusBar,
    WindowOpenProject,
    WindowExportBank,
    WindowBankEvents,
    WindowDiagnostics,
    WindowLog,
    WindowAbout,
    AboutText,
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
    SaveSucceeded {
        path: String,
    },
    SaveFailed {
        error: String,
    },
    SaveFailedLog {
        path: String,
        error: String,
    },
    AddedEventToBank {
        event_name: String,
        bank_name: String,
    },
    RemovedEventFromBank {
        event_name: String,
        bank_name: String,
    },
    CreatedEvent {
        event_name: String,
    },
    CreatedBus {
        bus_name: String,
    },
    CreatedSnapshot {
        snapshot_name: String,
    },
    CreatedParameter {
        parameter_name: String,
    },
    CreatedAsset {
        asset_name: String,
    },
    CreateAssetFailed {
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
        TextKey::OpenProject => "打开项目",
        TextKey::ProjectPath => "项目路径",
        TextKey::ProjectPathHint => "输入项目文件路径",
        TextKey::LoadProject => "加载项目",
        TextKey::RecentProjects => "最近项目",
        TextKey::Language => "语言",
        TextKey::ProjectPanel => "项目",
        TextKey::NoProjectLoaded => "尚未加载项目文件",
        TextKey::Name => "名称",
        TextKey::Assets => "资源",
        TextKey::Events => "事件",
        TextKey::Banks => "Bank",
        TextKey::BankList => "Bank列表",
        TextKey::QuickActions => "快捷操作",
        TextKey::CreateObjects => "创建对象",
        TextKey::NewAssetPath => "新资源路径",
        TextKey::NewParameterName => "新参数名称",
        TextKey::NewParameterVariants => "枚举值",
        TextKey::CreateAsset => "新建",
        TextKey::CreateEvent => "新建",
        TextKey::CreateBus => "新建",
        TextKey::CreateSnapshot => "新建",
        TextKey::CreateEnumParameter => "新建",
        TextKey::CreateEventNeedsAsset => "创建Event至少需要一个资源",
        TextKey::CreateParameterNeedsVariants => "创建参数至少需要一个枚举值",
        TextKey::LoadProjectFirst => "请先加载一个项目文件",
        TextKey::SelectBankFirst => "请先从左侧选择一个Bank",
        TextKey::SelectedBankMissing => "当前选中的Bank已不存在",
        TextKey::CurrentBank => "当前Bank",
        TextKey::EventCount => "事件数量",
        TextKey::BusCount => "总线数量",
        TextKey::SnapshotCount => "快照数量",
        TextKey::AssetCount => "资源数量",
        TextKey::ResidentMediaCount => "常驻资源数量",
        TextKey::StreamingMediaCount => "流式资源数量",
        TextKey::OutputPath => "输出路径",
        TextKey::OutputPathHint => "输入导出的compiled bank JSON路径",
        TextKey::ExportCompiledBank => "导出",
        TextKey::ResetDefaultExportPath => "重置路径为默认",
        TextKey::SaveProject => "保存项目",
        TextKey::UnsavedChanges => "有未保存变更",
        TextKey::Validation => "校验结果",
        TextKey::ValidationReady => "可导出",
        TextKey::ValidationBlocked => "不可导出",
        TextKey::NoValidationIssues => "未发现校验问题",
        TextKey::ValidationIssueCount => "问题数量",
        TextKey::LastExport => "最近一次导出",
        TextKey::NoExportYet => "尚未执行导出",
        TextKey::LastExportSuccess => "成功",
        TextKey::LastExportFailure => "失败",
        TextKey::FileSizeBytes => "文件大小",
        TextKey::WelcomeBack => "欢迎回来",
        TextKey::WelcomeTitle => "Sonara Editor",
        TextKey::WelcomeSubtitle => "Rust-first交互音频中间件编辑器",
        TextKey::ContinueProject => "继续当前项目",
        TextKey::NoRecentProjects => "暂无最近项目",
        TextKey::Inspector => "检查器",
        TextKey::NoProjectLoadedShort => "尚未加载项目",
        TextKey::NoBankSelected => "尚未选择Bank",
        TextKey::Buses => "总线",
        TextKey::Snapshots => "快照",
        TextKey::Parameters => "参数",
        TextKey::Kind => "类型",
        TextKey::Spatial => "空间化",
        TextKey::ParameterScope => "作用域",
        TextKey::ParameterVariants => "枚举值",
        TextKey::DefaultValue => "默认值",
        TextKey::VariantCount => "枚举值数量",
        TextKey::NodeCount => "节点数量",
        TextKey::ResolvedAssetCount => "引用资源数量",
        TextKey::BankContentsEditor => "Bank内容",
        TextKey::NoSelection => "未选择对象",
        TextKey::CurrentBankEvents => "当前Bank中的事件",
        TextKey::AvailableEvents => "可加入的事件",
        TextKey::CurrentBankBuses => "当前Bank中的总线",
        TextKey::AvailableBuses => "可加入的总线",
        TextKey::CurrentBankSnapshots => "当前Bank中的快照",
        TextKey::AvailableSnapshots => "可加入的快照",
        TextKey::AddToBank => "加入",
        TextKey::RemoveFromBank => "移出",
        TextKey::DeleteFromProject => "删除",
        TextKey::NoAvailableEvents => "没有可加入的事件",
        TextKey::NoAvailableBuses => "没有可加入的总线",
        TextKey::NoAvailableSnapshots => "没有可加入的快照",
        TextKey::DefaultVolume => "默认音量",
        TextKey::FadeInSeconds => "淡入秒数",
        TextKey::FadeOutSeconds => "淡出秒数",
        TextKey::EventContent => "事件内容",
        TextKey::ContentMode => "内容模式",
        TextKey::SingleAsset => "单资源",
        TextKey::StateSwitch => "按参数切换",
        TextKey::SwitchParameter => "切换参数",
        TextKey::SwitchVariants => "切换分支",
        TextKey::DefaultCase => "默认分支",
        TextKey::ConvertToSingleAsset => "转换为单资源",
        TextKey::ConvertToStateSwitch => "转换为按参数切换",
        TextKey::ProjectAssets => "项目资源",
        TextKey::ProjectParameters => "项目参数",
        TextKey::NoAssetsInProject => "无资源",
        TextKey::NoParameters => "无参数",
        TextKey::NoEnumParameters => "暂无可用的枚举参数",
        TextKey::EnumParameterHint => "枚举值使用半角逗号分隔",
        TextKey::AssetImportHint => "输入音频文件路径后导入到项目资源列表",
        TextKey::UnsupportedEventContent => "当前事件树暂不支持在此编辑",
        TextKey::UnsupportedParameterType => "当前参数类型暂不支持在此编辑",
        TextKey::NoEvents => "无事件",
        TextKey::NoBuses => "无总线",
        TextKey::NoSnapshots => "无快照",
        TextKey::Log => "日志",
        TextKey::Clear => "清空",
        TextKey::Confirm => "确定",
        TextKey::Cancel => "取消",
        TextKey::NoLogs => "暂无日志",
        TextKey::FontLoaded => "中文字体加载成功",
        TextKey::Ready => "Ready",
        TextKey::SavedProjectStatus => "项目已保存",
        TextKey::CurrentProject => "当前项目",
        TextKey::ToolStrip => "工具栏",
        TextKey::PreviewUnavailable => "预览功能暂未接入",
        TextKey::MenuFile => "文件",
        TextKey::MenuView => "视图",
        TextKey::MenuWindow => "窗口",
        TextKey::MenuHelp => "帮助",
        TextKey::ToggleProjectPanel => "项目栏",
        TextKey::ToggleInspectorPanel => "检查器",
        TextKey::ToggleToolStrip => "工具栏",
        TextKey::ToggleStatusBar => "状态栏",
        TextKey::WindowOpenProject => "打开项目",
        TextKey::WindowExportBank => "导出Bank",
        TextKey::WindowBankEvents => "Bank Contents",
        TextKey::WindowDiagnostics => "Diagnostics",
        TextKey::WindowLog => "Log",
        TextKey::WindowAbout => "About",
        TextKey::AboutText => {
            "Sonara Editor最小壳子, 用于project.json到compiled bank的authoring工作流"
        }
    }
}

fn text_en_us(key: TextKey) -> &'static str {
    match key {
        TextKey::OpenProject => "Open Project",
        TextKey::ProjectPath => "Project Path",
        TextKey::ProjectPathHint => "Enter the path to project.json",
        TextKey::LoadProject => "Load Project",
        TextKey::RecentProjects => "Recent Projects",
        TextKey::Language => "Language",
        TextKey::ProjectPanel => "Project",
        TextKey::NoProjectLoaded => "No project.json loaded.",
        TextKey::Name => "Name",
        TextKey::Assets => "Assets",
        TextKey::Events => "Events",
        TextKey::Banks => "Banks",
        TextKey::BankList => "Bank List",
        TextKey::QuickActions => "Quick Actions",
        TextKey::CreateObjects => "Create Objects",
        TextKey::NewAssetPath => "New Asset Path",
        TextKey::NewParameterName => "New Parameter Name",
        TextKey::NewParameterVariants => "Variants",
        TextKey::CreateAsset => "New",
        TextKey::CreateEvent => "New",
        TextKey::CreateBus => "New",
        TextKey::CreateSnapshot => "New",
        TextKey::CreateEnumParameter => "New",
        TextKey::CreateEventNeedsAsset => "Creating an event requires at least one asset",
        TextKey::CreateParameterNeedsVariants => {
            "Creating a parameter requires at least one enum variant"
        }
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
        TextKey::ExportCompiledBank => "Export",
        TextKey::ResetDefaultExportPath => "Reset Path to Default",
        TextKey::SaveProject => "Save Project",
        TextKey::UnsavedChanges => "Unsaved Changes",
        TextKey::Validation => "Validation",
        TextKey::ValidationReady => "Ready",
        TextKey::ValidationBlocked => "Blocked",
        TextKey::NoValidationIssues => "No validation issues found.",
        TextKey::ValidationIssueCount => "Issue Count",
        TextKey::LastExport => "Last Export",
        TextKey::NoExportYet => "No export has been run yet.",
        TextKey::LastExportSuccess => "Success",
        TextKey::LastExportFailure => "Failure",
        TextKey::FileSizeBytes => "File Size",
        TextKey::WelcomeBack => "Welcome Back",
        TextKey::WelcomeTitle => "Sonara Editor",
        TextKey::WelcomeSubtitle => "Rust-first interactive audio middleware editor",
        TextKey::ContinueProject => "Continue Current Project",
        TextKey::NoRecentProjects => "No recent projects.",
        TextKey::Inspector => "Inspector",
        TextKey::NoProjectLoadedShort => "No project loaded.",
        TextKey::NoBankSelected => "No bank selected.",
        TextKey::Buses => "Buses",
        TextKey::Snapshots => "Snapshots",
        TextKey::Parameters => "Parameters",
        TextKey::Kind => "Kind",
        TextKey::Spatial => "Spatial",
        TextKey::ParameterScope => "Scope",
        TextKey::ParameterVariants => "Variants",
        TextKey::DefaultValue => "Default Value",
        TextKey::VariantCount => "Variant Count",
        TextKey::NodeCount => "Node Count",
        TextKey::ResolvedAssetCount => "Referenced Asset Count",
        TextKey::BankContentsEditor => "Bank Contents",
        TextKey::NoSelection => "No selection",
        TextKey::CurrentBankEvents => "Events in Current Bank",
        TextKey::AvailableEvents => "Available Events",
        TextKey::CurrentBankBuses => "Buses in Current Bank",
        TextKey::AvailableBuses => "Available Buses",
        TextKey::CurrentBankSnapshots => "Snapshots in Current Bank",
        TextKey::AvailableSnapshots => "Available Snapshots",
        TextKey::AddToBank => "Add",
        TextKey::RemoveFromBank => "Remove",
        TextKey::DeleteFromProject => "Delete",
        TextKey::NoAvailableEvents => "No available events to add.",
        TextKey::NoAvailableBuses => "No available buses to add.",
        TextKey::NoAvailableSnapshots => "No available snapshots to add.",
        TextKey::DefaultVolume => "Default Volume",
        TextKey::FadeInSeconds => "Fade In Seconds",
        TextKey::FadeOutSeconds => "Fade Out Seconds",
        TextKey::EventContent => "Event Content",
        TextKey::ContentMode => "Content Mode",
        TextKey::SingleAsset => "Single Asset",
        TextKey::StateSwitch => "Switch by Parameter",
        TextKey::SwitchParameter => "Switch Parameter",
        TextKey::SwitchVariants => "Switch Cases",
        TextKey::DefaultCase => "Default Case",
        TextKey::ConvertToSingleAsset => "Convert to Single Asset",
        TextKey::ConvertToStateSwitch => "Convert to State Switch",
        TextKey::ProjectAssets => "Project Assets",
        TextKey::ProjectParameters => "Project Parameters",
        TextKey::NoAssetsInProject => "No assets.",
        TextKey::NoParameters => "No parameters.",
        TextKey::NoEnumParameters => "No enum parameters available.",
        TextKey::EnumParameterHint => "Separate enum variants with commas",
        TextKey::AssetImportHint => "Enter an audio file path to register it as a project asset",
        TextKey::UnsupportedEventContent => {
            "This event tree shape is not editable in the current inspector."
        }
        TextKey::UnsupportedParameterType => {
            "This parameter type is not editable in the current inspector."
        }
        TextKey::NoEvents => "No events.",
        TextKey::NoBuses => "No buses.",
        TextKey::NoSnapshots => "No snapshots.",
        TextKey::Log => "Log",
        TextKey::Clear => "Clear",
        TextKey::Confirm => "Confirm",
        TextKey::Cancel => "Cancel",
        TextKey::NoLogs => "No logs yet.",
        TextKey::FontLoaded => "Chinese font loaded successfully",
        TextKey::Ready => "Ready",
        TextKey::SavedProjectStatus => "Project Saved",
        TextKey::CurrentProject => "Current Project",
        TextKey::ToolStrip => "Tool Strip",
        TextKey::PreviewUnavailable => "Preview is not connected yet",
        TextKey::MenuFile => "File",
        TextKey::MenuView => "View",
        TextKey::MenuWindow => "Window",
        TextKey::MenuHelp => "Help",
        TextKey::ToggleProjectPanel => "Project Panel",
        TextKey::ToggleInspectorPanel => "Inspector",
        TextKey::ToggleToolStrip => "Tool Strip",
        TextKey::ToggleStatusBar => "Status Bar",
        TextKey::WindowOpenProject => "Open Project",
        TextKey::WindowExportBank => "Export Bank",
        TextKey::WindowBankEvents => "Bank Events",
        TextKey::WindowDiagnostics => "Diagnostics",
        TextKey::WindowLog => "Log",
        TextKey::WindowAbout => "About",
        TextKey::AboutText => {
            "Minimal Sonara Editor shell for the project.json to compiled bank authoring workflow"
        }
    }
}

fn template_zh_cn(template: TextTemplate) -> String {
    match template {
        TextTemplate::FontLoadFailed { error } => format!("中文字体加载失败: {error}"),
        TextTemplate::ProjectLoaded { path } => format!("已加载项目: {path}"),
        TextTemplate::LoadFailed { error } => format!("加载失败: {error}"),
        TextTemplate::LoadSucceeded { path } => format!("项目加载成功: {path}"),
        TextTemplate::SelectBank { bank_name } => format!("已选择Bank: {bank_name}"),
        TextTemplate::ExportFailedNoProject => "导出失败: 尚未加载项目".to_owned(),
        TextTemplate::ExportFailedNoBank => "导出失败: 尚未选择Bank".to_owned(),
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
        } => format!("导出成功, Bank={bank_name}, 事件数={event_count}, 输出={output_path}"),
        TextTemplate::ExportFailedLog {
            bank_name,
            output_path,
            error,
        } => format!("导出失败, Bank={bank_name}, 输出={output_path}, 错误={error}"),
        TextTemplate::LoadFailedLog { path, error } => {
            format!("加载项目失败, 路径={path}, 错误={error}")
        }
        TextTemplate::SaveSucceeded { path } => format!("项目保存成功: {path}"),
        TextTemplate::SaveFailed { error } => format!("项目保存失败: {error}"),
        TextTemplate::SaveFailedLog { path, error } => {
            format!("保存项目失败, 路径={path}, 错误={error}")
        }
        TextTemplate::AddedEventToBank {
            event_name,
            bank_name,
        } => format!("已将事件{event_name}加入Bank {bank_name}"),
        TextTemplate::RemovedEventFromBank {
            event_name,
            bank_name,
        } => format!("已将事件{event_name}从Bank {bank_name}移除"),
        TextTemplate::CreatedEvent { event_name } => format!("已创建Event {event_name}"),
        TextTemplate::CreatedBus { bus_name } => format!("已创建Bus {bus_name}"),
        TextTemplate::CreatedSnapshot { snapshot_name } => {
            format!("已创建Snapshot {snapshot_name}")
        }
        TextTemplate::CreatedParameter { parameter_name } => {
            format!("已创建枚举参数 {parameter_name}")
        }
        TextTemplate::CreatedAsset { asset_name } => format!("已导入资源 {asset_name}"),
        TextTemplate::CreateAssetFailed { error } => format!("导入资源失败: {error}"),
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
        TextTemplate::SaveSucceeded { path } => format!("Project saved: {path}"),
        TextTemplate::SaveFailed { error } => format!("Save failed: {error}"),
        TextTemplate::SaveFailedLog { path, error } => {
            format!("Failed to save project, path={path}, error={error}")
        }
        TextTemplate::AddedEventToBank {
            event_name,
            bank_name,
        } => format!("Added event {event_name} to bank {bank_name}"),
        TextTemplate::RemovedEventFromBank {
            event_name,
            bank_name,
        } => format!("Removed event {event_name} from bank {bank_name}"),
        TextTemplate::CreatedEvent { event_name } => format!("Created event {event_name}"),
        TextTemplate::CreatedBus { bus_name } => format!("Created bus {bus_name}"),
        TextTemplate::CreatedSnapshot { snapshot_name } => {
            format!("Created snapshot {snapshot_name}")
        }
        TextTemplate::CreatedParameter { parameter_name } => {
            format!("Created enum parameter {parameter_name}")
        }
        TextTemplate::CreatedAsset { asset_name } => format!("Imported asset {asset_name}"),
        TextTemplate::CreateAssetFailed { error } => format!("Failed to import asset: {error}"),
    }
}
