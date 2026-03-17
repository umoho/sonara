// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use sonara_model::AuthoringProject;

use crate::{
    compile::{compile_bank_definition, compile_bank_definition_to_file},
    error::{ProjectBuildError, ProjectExportBankError},
    package::CompiledBankPackage,
};

/// 从一个已加载的 project 中按 bank 名称编译 compiled bank。
pub fn compile_project_bank(
    project: &AuthoringProject,
    bank_name: &str,
) -> Result<CompiledBankPackage, ProjectBuildError> {
    let definition = project
        .bank_named(bank_name)
        .ok_or_else(|| ProjectBuildError::MissingBankDefinition(bank_name.to_owned()))?;
    Ok(compile_bank_definition(definition, project)?)
}

/// 从磁盘上的 project 文件中按 bank 名称编译 compiled bank。
pub fn compile_project_bank_file(
    project_path: impl AsRef<Path>,
    bank_name: &str,
) -> Result<CompiledBankPackage, ProjectBuildError> {
    let project = AuthoringProject::read_json_file(project_path)?;
    compile_project_bank(&project, bank_name)
}

/// 从一个已加载的 project 中按 bank 名称导出 compiled bank 文件。
pub fn compile_project_bank_to_file(
    project: &AuthoringProject,
    bank_name: &str,
    output_path: impl AsRef<Path>,
) -> Result<CompiledBankPackage, ProjectExportBankError> {
    let definition = project
        .bank_named(bank_name)
        .ok_or_else(|| ProjectExportBankError::MissingBankDefinition(bank_name.to_owned()))?;
    Ok(compile_bank_definition_to_file(
        definition,
        project,
        output_path,
    )?)
}

/// 从磁盘上的 project 文件中按 bank 名称导出 compiled bank 文件。
pub fn compile_project_bank_file_to_file(
    project_path: impl AsRef<Path>,
    bank_name: &str,
    output_path: impl AsRef<Path>,
) -> Result<CompiledBankPackage, ProjectExportBankError> {
    let project = AuthoringProject::read_json_file(project_path)?;
    compile_project_bank_to_file(&project, bank_name, output_path)
}
