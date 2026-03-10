//! Firewheel 后端适配层

use sonara_runtime::SonaraRuntime;

/// 基于 Firewheel 的运行时后端骨架
#[derive(Debug, Default)]
pub struct FirewheelBackend {
    runtime: SonaraRuntime,
}

impl FirewheelBackend {
    /// 使用现有运行时创建后端
    pub fn new(runtime: SonaraRuntime) -> Self {
        Self { runtime }
    }

    /// 获取后端持有的运行时引用
    pub fn runtime(&self) -> &SonaraRuntime {
        &self.runtime
    }
}
