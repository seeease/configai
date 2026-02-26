use std::path::{Path, PathBuf};

use crate::error::{ConfigError, Result};
use crate::models::{ConfigState, SharedGroup};

/// 存储引擎：内存状态 + JSON 文件持久化
pub struct Storage {
    state: ConfigState,
    file_path: PathBuf,
}

impl Storage {
    /// 从 JSON 文件加载状态。文件不存在则初始化空状态，文件损坏则记录错误并初始化空状态。
    pub fn load(file_path: &Path) -> Result<Self> {
        let state = if file_path.exists() {
            match std::fs::read_to_string(file_path) {
                Ok(content) => match serde_json::from_str::<ConfigState>(&content) {
                    Ok(state) => state,
                    Err(e) => {
                        tracing::warn!("配置文件损坏，初始化空状态: {}", e);
                        Self::empty_state()
                    }
                },
                Err(e) => {
                    tracing::warn!("无法读取配置文件，初始化空状态: {}", e);
                    Self::empty_state()
                }
            }
        } else {
            Self::empty_state()
        };

        Ok(Self {
            state,
            file_path: file_path.to_path_buf(),
        })
    }

    /// 将内存状态序列化为 JSON 写入文件
    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.state)
            .map_err(|e| ConfigError::StorageError(e.to_string()))?;

        // 确保父目录存在
        if let Some(parent) = self.file_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        std::fs::write(&self.file_path, json)?;
        Ok(())
    }

    /// 获取状态的不可变引用
    pub fn state(&self) -> &ConfigState {
        &self.state
    }

    /// 获取状态的可变引用
    pub fn state_mut(&mut self) -> &mut ConfigState {
        &mut self.state
    }

    /// 创建空的初始状态
    fn empty_state() -> ConfigState {
        ConfigState {
            projects: vec![],
            api_keys: vec![],
            shared_group: SharedGroup {
                environments: vec![],
            },
        }
    }
}
