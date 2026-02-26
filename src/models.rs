use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 项目元信息（从 project.yaml 加载）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProjectMeta {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub api_keys: Vec<ApiKeyEntry>,
}

/// API Key 条目
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKeyEntry {
    pub key: String,
}

/// 完整的内存状态（从目录扫描构建）
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigState {
    /// 项目名 -> 项目数据
    pub projects: HashMap<String, ProjectData>,
    /// 共享配置：环境名 -> 配置 KV
    pub shared: HashMap<String, HashMap<String, serde_json::Value>>,
}

/// 单个项目的数据
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectData {
    pub meta: ProjectMeta,
    /// 环境名 -> 配置 KV
    pub environments: HashMap<String, HashMap<String, serde_json::Value>>,
}
