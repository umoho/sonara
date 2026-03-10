use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// 导入后的底层音频资源
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioAsset {
    pub id: Uuid,
    pub name: SmolStr,
    pub source_path: Utf8PathBuf,
    pub import_settings: ImportSettings,
    pub streaming: StreamingMode,
    pub loop_region: Option<LoopRegion>,
    pub analysis: Option<AnalysisMetadata>,
}

impl AudioAsset {
    /// 使用最小默认设置创建一个音频资源
    pub fn new(name: impl Into<SmolStr>, source_path: Utf8PathBuf) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            source_path,
            import_settings: ImportSettings::default(),
            streaming: StreamingMode::Auto,
            loop_region: None,
            analysis: None,
        }
    }
}

/// 导入阶段的基础处理选项
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSettings {
    pub normalize: bool,
    pub target_sample_rate: Option<u32>,
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            normalize: false,
            target_sample_rate: None,
        }
    }
}

/// 资源的加载策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamingMode {
    /// 由编译器或导入规则自动决定
    Auto,
    /// 常驻内存
    Resident,
    /// 流式读取
    Streaming,
}

/// 音频资源中的循环区间
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRegion {
    pub start_frame: u64,
    pub end_frame: u64,
}

/// 供编辑器和构建流程使用的分析数据
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    pub duration_seconds: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub peak_dbfs: Option<f32>,
}
