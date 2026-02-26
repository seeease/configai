use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::{ConfigState, ProjectData, ProjectMeta};

/// 目录扫描式存储引擎
pub struct Storage {
    state: ConfigState,
    config_dir: PathBuf,
}

impl Storage {
    /// 从配置目录加载所有 YAML 文件
    pub fn load(config_dir: &Path) -> Result<Self> {
        let state = if config_dir.exists() {
            let projects = load_projects(&config_dir.join("projects"));
            let shared = load_shared(&config_dir.join("shared"));
            ConfigState { projects, shared }
        } else {
            ConfigState {
                projects: HashMap::new(),
                shared: HashMap::new(),
            }
        };

        Ok(Self {
            state,
            config_dir: config_dir.to_path_buf(),
        })
    }

    pub fn state(&self) -> &ConfigState {
        &self.state
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

/// 扫描 projects/ 目录，每个子目录是一个项目
fn load_projects(projects_dir: &Path) -> HashMap<String, ProjectData> {
    let mut projects = HashMap::new();
    let entries = match std::fs::read_dir(projects_dir) {
        Ok(e) => e,
        Err(_) => return projects,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let project_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let meta = load_project_meta(&path.join("project.yaml"));
        let environments = load_env_configs(&path);
        projects.insert(project_name, ProjectData { meta, environments });
    }

    projects
}

/// 加载 project.yaml → ProjectMeta
fn load_project_meta(path: &Path) -> ProjectMeta {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return ProjectMeta::default(),
    };
    match serde_yaml::from_str::<ProjectMeta>(&content) {
        Ok(meta) => meta,
        Err(e) => {
            tracing::warn!("解析 project.yaml 失败 {:?}: {}", path, e);
            ProjectMeta::default()
        }
    }
}

/// 扫描项目目录下的 *.yaml（排除 project.yaml），每个文件是一个环境
fn load_env_configs(project_dir: &Path) -> HashMap<String, HashMap<String, serde_json::Value>> {
    let mut envs = HashMap::new();
    let entries = match std::fs::read_dir(project_dir) {
        Ok(e) => e,
        Err(_) => return envs,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_yaml_file(&path) {
            continue;
        }
        let file_name = match path.file_stem().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        // 跳过 project.yaml
        if file_name == "project" {
            continue;
        }
        if let Some(map) = load_yaml_map(&path) {
            envs.insert(file_name, map);
        }
    }

    envs
}

/// 扫描 shared/ 目录，每个 *.yaml 是一个环境的共享配置
fn load_shared(shared_dir: &Path) -> HashMap<String, HashMap<String, serde_json::Value>> {
    let mut shared = HashMap::new();
    let entries = match std::fs::read_dir(shared_dir) {
        Ok(e) => e,
        Err(_) => return shared,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_yaml_file(&path) {
            continue;
        }
        let env_name = match path.file_stem().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if let Some(map) = load_yaml_map(&path) {
            shared.insert(env_name, map);
        }
    }

    shared
}

/// 加载 YAML 文件为 HashMap<String, serde_json::Value>
fn load_yaml_map(path: &Path) -> Option<HashMap<String, serde_json::Value>> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("读取文件失败 {:?}: {}", path, e);
            return None;
        }
    };
    // serde_yaml -> serde_yaml::Value -> serde_json::Value 转换
    let yaml_value: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("解析 YAML 失败 {:?}: {}", path, e);
            return None;
        }
    };
    let json_value = yaml_to_json(yaml_value);
    match json_value {
        serde_json::Value::Object(map) => {
            Some(map.into_iter().collect())
        }
        _ => {
            tracing::warn!("YAML 文件顶层不是 mapping {:?}", path);
            None
        }
    }
}

/// 递归将 serde_yaml::Value 转换为 serde_json::Value
fn yaml_to_json(yaml: serde_yaml::Value) -> serde_json::Value {
    match yaml {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::json!(f)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        serde_yaml::Value::String(s) => s,
                        other => serde_json::to_string(&yaml_to_json(other)).ok()?,
                    };
                    Some((key, yaml_to_json(v)))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
}

fn is_yaml_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
}
