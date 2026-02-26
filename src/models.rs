use serde::{Deserialize, Serialize};

/// 完整的配置状态，用于内存存储和文件持久化
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigState {
    pub projects: Vec<Project>,
    pub shared_group: SharedGroup,
    pub api_keys: Vec<ApiKey>,
}

/// 项目
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub name: String,
    pub description: Option<String>,
    pub environments: Vec<Environment>,
}

/// 环境
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Environment {
    pub name: String,
    pub config_items: Vec<ConfigItem>,
}

/// 配置项（value 支持任意 JSON 值）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigItem {
    pub key: String,
    pub value: serde_json::Value,
}

/// 公共配置组
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SharedGroup {
    pub environments: Vec<Environment>,
}

/// API Key
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKey {
    pub key: String,     // UUID v4
    pub project: String, // 绑定的项目名
}
