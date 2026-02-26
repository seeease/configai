pub mod api_key;
pub mod config;
pub mod env;
pub mod project;
pub mod shared;

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;
use crate::models::{ApiKey, ConfigItem, Environment, Project};
use crate::storage::Storage;

/// 配置中心：组装项目和环境管理，委托给各子模块的自由函数
pub struct ConfigCenter {
    storage: Storage,
}

impl ConfigCenter {
    /// 从文件路径创建 ConfigCenter 实例
    pub fn new(file_path: &Path) -> Result<Self> {
        let storage = Storage::load(file_path)?;
        Ok(Self { storage })
    }

    /// 获取 Storage 的不可变引用
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// 获取 Storage 的可变引用
    pub fn storage_mut(&mut self) -> &mut Storage {
        &mut self.storage
    }

    // ---- 项目管理 ----

    pub fn create_project(&mut self, name: &str, description: Option<&str>) -> Result<Project> {
        project::create_project(&mut self.storage, name, description)
    }

    pub fn list_projects(&self) -> Vec<&Project> {
        project::list_projects(&self.storage)
    }

    pub fn delete_project(&mut self, name: &str) -> Result<()> {
        project::delete_project(&mut self.storage, name)
    }

    // ---- 环境管理 ----

    pub fn create_environment(&mut self, project: &str, env_name: &str) -> Result<Environment> {
        env::create_environment(&mut self.storage, project, env_name)
    }

    pub fn list_environments(&self, project: &str) -> Result<Vec<&Environment>> {
        env::list_environments(&self.storage, project)
    }

    pub fn delete_environment(&mut self, project: &str, env_name: &str) -> Result<()> {
        env::delete_environment(&mut self.storage, project, env_name)
    }

    // ---- 配置项管理 ----

    pub fn create_config_item(
        &mut self,
        project: &str,
        env: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<ConfigItem> {
        config::create_config_item(&mut self.storage, project, env, key, value)
    }

    pub fn update_config_item(
        &mut self,
        project: &str,
        env: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<ConfigItem> {
        config::update_config_item(&mut self.storage, project, env, key, value)
    }

    pub fn delete_config_item(&mut self, project: &str, env: &str, key: &str) -> Result<()> {
        config::delete_config_item(&mut self.storage, project, env, key)
    }

    pub fn list_config_items(&self, project: &str, env: &str) -> Result<Vec<&ConfigItem>> {
        config::list_config_items(&self.storage, project, env)
    }

    // ---- 公共配置组管理 ----

    pub fn create_shared_item(
        &mut self,
        env: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<ConfigItem> {
        shared::create_shared_item(&mut self.storage, env, key, value)
    }

    pub fn update_shared_item(
        &mut self,
        env: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<ConfigItem> {
        shared::update_shared_item(&mut self.storage, env, key, value)
    }

    pub fn delete_shared_item(&mut self, env: &str, key: &str) -> Result<()> {
        shared::delete_shared_item(&mut self.storage, env, key)
    }

    pub fn list_shared_items(&self, env: &str) -> Result<Vec<&ConfigItem>> {
        shared::list_shared_items(&self.storage, env)
    }

    // ---- API Key 管理 ----

    pub fn generate_api_key(&mut self, project: &str) -> Result<ApiKey> {
        api_key::generate_api_key(&mut self.storage, project)
    }

    pub fn revoke_api_key(&mut self, key: &str) -> Result<()> {
        api_key::revoke_api_key(&mut self.storage, key)
    }

    pub fn list_api_keys(&self, project: &str) -> Result<Vec<&ApiKey>> {
        api_key::list_api_keys(&self.storage, project)
    }

    pub fn validate_api_key(&self, key: &str) -> Result<&ApiKey> {
        api_key::validate_api_key(&self.storage, key)
    }

    // ---- 导入导出 ----

    /// 将完整 ConfigState 序列化为 pretty-printed JSON 字符串
    pub fn export_all(&self) -> Result<String> {
        serde_json::to_string_pretty(self.storage.state()).map_err(|e| e.into())
    }

    /// 从 JSON 字符串反序列化并替换当前状态，持久化到文件。
    /// 如果持久化失败，不修改内存状态（回滚）。
    pub fn import_all(&mut self, json: &str) -> Result<()> {
        let new_state: crate::models::ConfigState =
            serde_json::from_str(json).map_err(crate::error::ConfigError::from)?;

        // 暂存旧状态，替换为新状态
        let old_state = std::mem::replace(self.storage.state_mut(), new_state);

        // 尝试持久化
        if let Err(e) = self.storage.save() {
            // 回滚
            *self.storage.state_mut() = old_state;
            return Err(e);
        }

        Ok(())
    }

    // ---- 合并配置 ----

    pub fn get_merged_config(
        &self,
        project: &str,
        env: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        shared::get_merged_config(&self.storage, project, env)
    }

    pub fn get_merged_config_item(
        &self,
        project: &str,
        env: &str,
        key: &str,
    ) -> Result<serde_json::Value> {
        shared::get_merged_config_item(&self.storage, project, env, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ConfigError;
    use tempfile::NamedTempFile;

    fn test_center() -> ConfigCenter {
        let tmp = NamedTempFile::new().unwrap();
        ConfigCenter::new(tmp.path()).unwrap()
    }

    #[test]
    fn test_create_and_list_projects() {
        let mut center = test_center();
        assert!(center.list_projects().is_empty());

        let p = center.create_project("app", Some("my app")).unwrap();
        assert_eq!(p.name, "app");
        assert_eq!(p.description, Some("my app".to_string()));

        let projects = center.list_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "app");
    }

    #[test]
    fn test_delete_project() {
        let mut center = test_center();
        center.create_project("app", None).unwrap();
        center.delete_project("app").unwrap();
        assert!(center.list_projects().is_empty());
    }

    #[test]
    fn test_create_and_list_environments() {
        let mut center = test_center();
        center.create_project("app", None).unwrap();

        let env = center.create_environment("app", "staging").unwrap();
        assert_eq!(env.name, "staging");

        let envs = center.list_environments("app").unwrap();
        assert_eq!(envs.len(), 2); // default + staging
    }

    #[test]
    fn test_delete_environment() {
        let mut center = test_center();
        center.create_project("app", None).unwrap();
        center.create_environment("app", "staging").unwrap();
        center.delete_environment("app", "staging").unwrap();

        let envs = center.list_environments("app").unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "default");
    }

    #[test]
    fn test_project_not_found() {
        let mut center = test_center();
        let err = center.delete_project("nope").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_environment_project_not_found() {
        let mut center = test_center();
        let err = center.create_environment("nope", "dev").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_export_produces_valid_json() {
        let mut center = test_center();
        center.create_project("app", Some("desc")).unwrap();
        center
            .create_config_item("app", "default", "key1", serde_json::json!("value1"))
            .unwrap();

        let json = center.export_all().unwrap();
        // 验证导出的 JSON 可以被解析
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_object());
        assert!(parsed.get("projects").is_some());
        assert!(parsed.get("shared_group").is_some());
        assert!(parsed.get("api_keys").is_some());
    }

    #[test]
    fn test_import_replaces_state() {
        let mut center = test_center();
        center.create_project("old", None).unwrap();
        assert_eq!(center.list_projects().len(), 1);

        // 构造一个包含不同项目的 JSON
        let import_json = serde_json::json!({
            "projects": [
                {
                    "name": "new-proj",
                    "description": null,
                    "environments": [
                        { "name": "default", "config_items": [] }
                    ]
                }
            ],
            "shared_group": { "environments": [] },
            "api_keys": []
        });

        center.import_all(&import_json.to_string()).unwrap();
        let projects = center.list_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "new-proj");
    }

    #[test]
    fn test_export_import_roundtrip() {
        let mut center = test_center();
        center.create_project("app", Some("my app")).unwrap();
        center
            .create_config_item("app", "default", "db_host", serde_json::json!("localhost"))
            .unwrap();
        center
            .create_config_item("app", "default", "db_port", serde_json::json!(5432))
            .unwrap();
        // shared_group 需要先手动创建环境
        center
            .storage_mut()
            .state_mut()
            .shared_group
            .environments
            .push(crate::models::Environment {
                name: "default".to_string(),
                config_items: vec![],
            });
        center
            .create_shared_item("default", "log_level", serde_json::json!("info"))
            .unwrap();
        center.generate_api_key("app").unwrap();

        // 导出
        let exported = center.export_all().unwrap();

        // 创建新的 center 并导入
        let mut center2 = test_center();
        center2.import_all(&exported).unwrap();

        // 再次导出，比较两次导出的反序列化结果
        let exported2 = center2.export_all().unwrap();
        let state1: crate::models::ConfigState = serde_json::from_str(&exported).unwrap();
        let state2: crate::models::ConfigState = serde_json::from_str(&exported2).unwrap();
        assert_eq!(state1, state2);
    }

    #[test]
    fn test_storage_accessors() {
        let mut center = test_center();
        center.create_project("app", None).unwrap();

        // 通过 storage() 直接访问状态
        assert_eq!(center.storage().state().projects.len(), 1);

        // 通过 storage_mut() 可修改
        center.storage_mut().state_mut().projects.clear();
        assert!(center.list_projects().is_empty());
    }
}
