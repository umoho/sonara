// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::ids::ParameterId;

/// 事件系统中的参数定义
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Parameter {
    Float(FloatParameter),
    Bool(BoolParameter),
    Enum(EnumParameter),
}

impl Parameter {
    /// 获取参数 ID
    pub fn id(&self) -> ParameterId {
        match self {
            Self::Float(parameter) => parameter.id,
            Self::Bool(parameter) => parameter.id,
            Self::Enum(parameter) => parameter.id,
        }
    }

    /// 获取参数名
    pub fn name(&self) -> &SmolStr {
        match self {
            Self::Float(parameter) => &parameter.name,
            Self::Bool(parameter) => &parameter.name,
            Self::Enum(parameter) => &parameter.name,
        }
    }
}

/// 参数作用域
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterScope {
    Global,
    Emitter,
    EventInstance,
}

/// 参数类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterKind {
    Float,
    Bool,
    Enum,
}

/// 参数默认值的统一表示
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterDefaultValue {
    Float(f32),
    Bool(bool),
    Enum(SmolStr),
}

/// 运行时参数值
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    Float(f32),
    Bool(bool),
    Enum(SmolStr),
}

/// 浮点参数
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FloatParameter {
    pub id: ParameterId,
    pub name: SmolStr,
    pub scope: ParameterScope,
    pub default_value: f32,
    pub min: f32,
    pub max: f32,
    pub smoothing_seconds: Option<f32>,
}

/// 布尔参数
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoolParameter {
    pub id: ParameterId,
    pub name: SmolStr,
    pub scope: ParameterScope,
    pub default_value: bool,
}

/// 枚举参数
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumParameter {
    pub id: ParameterId,
    pub name: SmolStr,
    pub scope: ParameterScope,
    pub default_value: SmolStr,
    pub variants: Vec<SmolStr>,
}
