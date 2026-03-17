// SPDX-License-Identifier: MPL-2.0

//! Sonara 的构建层
//!
//! 这一层负责 authoring 数据校验和 bank 构建

mod compile;
mod error;
mod media;
mod package;
mod project;
mod validate;

pub use compile::{
    build_bank, build_bank_from_definition, compile_bank_definition,
    compile_bank_definition_to_file,
};
pub use error::{
    BuildError, CompiledBankFileError, ExportBankError, ProjectBuildError, ProjectExportBankError,
};
pub use package::CompiledBankPackage;
pub use project::{
    compile_project_bank, compile_project_bank_file, compile_project_bank_file_to_file,
    compile_project_bank_to_file,
};
pub use validate::{collect_event_asset_ids, validate_event};

#[cfg(test)]
mod tests;
