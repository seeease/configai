use std::collections::HashMap;

use crate::error::{ConfigError, Result};
use crate::models::ConfigItem;
use crate::storage::Storage;

/// 在公共配置组的指定环境下创建配置项。
/// 写时持久化，失败回滚。
pub fn create_shared_item(
    storage: &mut Storage,
    env: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<ConfigItem> {
    // 验证环境存在
    let environment = storage
        .state()
        .shared_group
        .environments
        .iter()
        .find(|e| e.name == env)
        .ok_or_else(|| ConfigError::EnvironmentNotFound(env.to_string()))?;

    // 检查键唯一性
    if environment.config_items.iter().any(|item| item.key == key) {
        return Err(ConfigError::ConfigItemAlreadyExists(key.to_string()));
    }

    let config_item = ConfigItem {
        key: key.to_string(),
        value,
    };

    // 写时持久化
    storage
        .state_mut()
        .shared_group
        .environments
        .iter_mut()
        .find(|e| e.name == env)
        .unwrap()
        .config_items
        .push(config_item.clone());

    if let Err(e) = storage.save() {
        storage
            .state_mut()
            .shared_group
            .environments
            .iter_mut()
            .find(|e| e.name == env)
            .unwrap()
            .config_items
            .pop();
        return Err(e);
    }

    Ok(config_item)
}


/// 更新公共配置组中的配置项值。写时持久化，失败回滚。
pub fn update_shared_item(
    storage: &mut Storage,
    env: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<ConfigItem> {
    // 验证环境存在
    let environment = storage
        .state()
        .shared_group
        .environments
        .iter()
        .find(|e| e.name == env)
        .ok_or_else(|| ConfigError::EnvironmentNotFound(env.to_string()))?;

    // 验证键存在
    if !environment.config_items.iter().any(|item| item.key == key) {
        return Err(ConfigError::ConfigItemNotFound(key.to_string()));
    }

    // 写时持久化
    let item = storage
        .state_mut()
        .shared_group
        .environments
        .iter_mut()
        .find(|e| e.name == env)
        .unwrap()
        .config_items
        .iter_mut()
        .find(|item| item.key == key)
        .unwrap();

    let old_value = item.value.clone();
    item.value = value.clone();

    if let Err(e) = storage.save() {
        let item = storage
            .state_mut()
            .shared_group
            .environments
            .iter_mut()
            .find(|e| e.name == env)
            .unwrap()
            .config_items
            .iter_mut()
            .find(|item| item.key == key)
            .unwrap();
        item.value = old_value;
        return Err(e);
    }

    Ok(ConfigItem {
        key: key.to_string(),
        value,
    })
}

/// 删除公共配置组中的配置项。写时持久化，失败回滚。
pub fn delete_shared_item(storage: &mut Storage, env: &str, key: &str) -> Result<()> {
    // 验证环境存在
    let environment = storage
        .state()
        .shared_group
        .environments
        .iter()
        .find(|e| e.name == env)
        .ok_or_else(|| ConfigError::EnvironmentNotFound(env.to_string()))?;

    // 验证键存在
    let pos = environment
        .config_items
        .iter()
        .position(|item| item.key == key)
        .ok_or_else(|| ConfigError::ConfigItemNotFound(key.to_string()))?;

    // 写时持久化
    let removed = storage
        .state_mut()
        .shared_group
        .environments
        .iter_mut()
        .find(|e| e.name == env)
        .unwrap()
        .config_items
        .remove(pos);

    if let Err(e) = storage.save() {
        storage
            .state_mut()
            .shared_group
            .environments
            .iter_mut()
            .find(|e| e.name == env)
            .unwrap()
            .config_items
            .insert(pos, removed);
        return Err(e);
    }

    Ok(())
}

/// 列出公共配置组指定环境下的所有配置项
pub fn list_shared_items<'a>(storage: &'a Storage, env: &str) -> Result<Vec<&'a ConfigItem>> {
    let environment = storage
        .state()
        .shared_group
        .environments
        .iter()
        .find(|e| e.name == env)
        .ok_or_else(|| ConfigError::EnvironmentNotFound(env.to_string()))?;

    Ok(environment.config_items.iter().collect())
}

/// 合并项目配置和公共配置，项目配置优先覆盖。
/// 1. 验证项目存在
/// 2. 验证环境存在于项目中
/// 3. 从 shared_group 中取同名环境的配置（如果存在）
/// 4. 用项目配置覆盖
pub fn get_merged_config(
    storage: &Storage,
    project: &str,
    env: &str,
) -> Result<HashMap<String, serde_json::Value>> {
    // 验证项目存在
    let proj = storage
        .state()
        .projects
        .iter()
        .find(|p| p.name == project)
        .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

    // 验证环境存在于项目中
    let proj_env = proj
        .environments
        .iter()
        .find(|e| e.name == env)
        .ok_or_else(|| ConfigError::EnvironmentNotFound(env.to_string()))?;

    let mut merged = HashMap::new();

    // 先加载 shared_group 中同名环境的配置（如果存在）
    if let Some(shared_env) = storage
        .state()
        .shared_group
        .environments
        .iter()
        .find(|e| e.name == env)
    {
        for item in &shared_env.config_items {
            merged.insert(item.key.clone(), item.value.clone());
        }
    }

    // 项目配置覆盖
    for item in &proj_env.config_items {
        merged.insert(item.key.clone(), item.value.clone());
    }

    Ok(merged)
}

/// 获取合并后的单个配置项
pub fn get_merged_config_item(
    storage: &Storage,
    project: &str,
    env: &str,
    key: &str,
) -> Result<serde_json::Value> {
    let merged = get_merged_config(storage, project, env)?;
    merged
        .get(key)
        .cloned()
        .ok_or_else(|| ConfigError::ConfigItemNotFound(key.to_string()))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::create_config_item;
    use crate::core::project::create_project;
    use crate::models::Environment;
    use tempfile::NamedTempFile;

    fn test_storage() -> Storage {
        let tmp = NamedTempFile::new().unwrap();
        Storage::load(tmp.path()).unwrap()
    }

    /// 辅助：在 shared_group 中创建环境
    fn setup_shared_env(storage: &mut Storage, env_name: &str) {
        storage
            .state_mut()
            .shared_group
            .environments
            .push(Environment {
                name: env_name.to_string(),
                config_items: vec![],
            });
        storage.save().unwrap();
    }

    // ---- Shared CRUD ----

    #[test]
    fn test_create_shared_item() {
        let mut storage = test_storage();
        setup_shared_env(&mut storage, "default");

        let item =
            create_shared_item(&mut storage, "default", "log_level", serde_json::json!("info"))
                .unwrap();
        assert_eq!(item.key, "log_level");
        assert_eq!(item.value, serde_json::json!("info"));

        let items = list_shared_items(&storage, "default").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].key, "log_level");
    }

    #[test]
    fn test_create_shared_item_duplicate_key() {
        let mut storage = test_storage();
        setup_shared_env(&mut storage, "default");

        create_shared_item(&mut storage, "default", "k", serde_json::json!("v")).unwrap();
        let err =
            create_shared_item(&mut storage, "default", "k", serde_json::json!("v2")).unwrap_err();
        assert!(matches!(err, ConfigError::ConfigItemAlreadyExists(_)));
    }

    #[test]
    fn test_create_shared_item_env_not_found() {
        let mut storage = test_storage();
        let err =
            create_shared_item(&mut storage, "nope", "k", serde_json::json!("v")).unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_update_shared_item() {
        let mut storage = test_storage();
        setup_shared_env(&mut storage, "default");
        create_shared_item(&mut storage, "default", "k", serde_json::json!("old")).unwrap();

        let updated =
            update_shared_item(&mut storage, "default", "k", serde_json::json!(42)).unwrap();
        assert_eq!(updated.value, serde_json::json!(42));

        let items = list_shared_items(&storage, "default").unwrap();
        assert_eq!(items[0].value, serde_json::json!(42));
    }

    #[test]
    fn test_update_shared_item_not_found() {
        let mut storage = test_storage();
        setup_shared_env(&mut storage, "default");

        let err =
            update_shared_item(&mut storage, "default", "nope", serde_json::json!("v")).unwrap_err();
        assert!(matches!(err, ConfigError::ConfigItemNotFound(_)));
    }

    #[test]
    fn test_update_shared_item_env_not_found() {
        let mut storage = test_storage();
        let err =
            update_shared_item(&mut storage, "nope", "k", serde_json::json!("v")).unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_delete_shared_item() {
        let mut storage = test_storage();
        setup_shared_env(&mut storage, "default");
        create_shared_item(&mut storage, "default", "k", serde_json::json!("v")).unwrap();

        delete_shared_item(&mut storage, "default", "k").unwrap();
        assert!(list_shared_items(&storage, "default").unwrap().is_empty());
    }

    #[test]
    fn test_delete_shared_item_not_found() {
        let mut storage = test_storage();
        setup_shared_env(&mut storage, "default");

        let err = delete_shared_item(&mut storage, "default", "nope").unwrap_err();
        assert!(matches!(err, ConfigError::ConfigItemNotFound(_)));
    }

    #[test]
    fn test_delete_shared_item_env_not_found() {
        let mut storage = test_storage();
        let err = delete_shared_item(&mut storage, "nope", "k").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_list_shared_items_empty() {
        let mut storage = test_storage();
        setup_shared_env(&mut storage, "default");

        let items = list_shared_items(&storage, "default").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_list_shared_items_env_not_found() {
        let storage = test_storage();
        let err = list_shared_items(&storage, "nope").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    // ---- Merge ----

    #[test]
    fn test_get_merged_config_project_only() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        create_config_item(
            &mut storage,
            "app",
            "default",
            "db_host",
            serde_json::json!("localhost"),
        )
        .unwrap();

        let merged = get_merged_config(&storage, "app", "default").unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged["db_host"], serde_json::json!("localhost"));
    }

    #[test]
    fn test_get_merged_config_shared_only() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        setup_shared_env(&mut storage, "default");
        create_shared_item(
            &mut storage,
            "default",
            "log_level",
            serde_json::json!("info"),
        )
        .unwrap();

        let merged = get_merged_config(&storage, "app", "default").unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged["log_level"], serde_json::json!("info"));
    }

    #[test]
    fn test_get_merged_config_project_overrides_shared() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        setup_shared_env(&mut storage, "default");

        // shared: log_level = "info"
        create_shared_item(
            &mut storage,
            "default",
            "log_level",
            serde_json::json!("info"),
        )
        .unwrap();
        // shared: timeout = 30
        create_shared_item(
            &mut storage,
            "default",
            "timeout",
            serde_json::json!(30),
        )
        .unwrap();

        // project: log_level = "debug" (覆盖 shared)
        create_config_item(
            &mut storage,
            "app",
            "default",
            "log_level",
            serde_json::json!("debug"),
        )
        .unwrap();
        // project: db_host (仅项目)
        create_config_item(
            &mut storage,
            "app",
            "default",
            "db_host",
            serde_json::json!("localhost"),
        )
        .unwrap();

        let merged = get_merged_config(&storage, "app", "default").unwrap();
        assert_eq!(merged.len(), 3);
        // 项目覆盖 shared
        assert_eq!(merged["log_level"], serde_json::json!("debug"));
        // 仅 shared
        assert_eq!(merged["timeout"], serde_json::json!(30));
        // 仅项目
        assert_eq!(merged["db_host"], serde_json::json!("localhost"));
    }

    #[test]
    fn test_get_merged_config_no_shared_env() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        create_config_item(
            &mut storage,
            "app",
            "default",
            "k",
            serde_json::json!("v"),
        )
        .unwrap();

        // shared_group 中没有 "default" 环境，应该只返回项目配置
        let merged = get_merged_config(&storage, "app", "default").unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged["k"], serde_json::json!("v"));
    }

    #[test]
    fn test_get_merged_config_project_not_found() {
        let storage = test_storage();
        let err = get_merged_config(&storage, "nope", "default").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_get_merged_config_env_not_found() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let err = get_merged_config(&storage, "app", "nope").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_get_merged_config_item_from_project() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        create_config_item(
            &mut storage,
            "app",
            "default",
            "db_host",
            serde_json::json!("localhost"),
        )
        .unwrap();

        let val = get_merged_config_item(&storage, "app", "default", "db_host").unwrap();
        assert_eq!(val, serde_json::json!("localhost"));
    }

    #[test]
    fn test_get_merged_config_item_from_shared() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        setup_shared_env(&mut storage, "default");
        create_shared_item(
            &mut storage,
            "default",
            "log_level",
            serde_json::json!("info"),
        )
        .unwrap();

        let val = get_merged_config_item(&storage, "app", "default", "log_level").unwrap();
        assert_eq!(val, serde_json::json!("info"));
    }

    #[test]
    fn test_get_merged_config_item_project_overrides() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        setup_shared_env(&mut storage, "default");

        create_shared_item(
            &mut storage,
            "default",
            "log_level",
            serde_json::json!("info"),
        )
        .unwrap();
        create_config_item(
            &mut storage,
            "app",
            "default",
            "log_level",
            serde_json::json!("debug"),
        )
        .unwrap();

        let val = get_merged_config_item(&storage, "app", "default", "log_level").unwrap();
        assert_eq!(val, serde_json::json!("debug"));
    }

    #[test]
    fn test_get_merged_config_item_not_found() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let err = get_merged_config_item(&storage, "app", "default", "nope").unwrap_err();
        assert!(matches!(err, ConfigError::ConfigItemNotFound(_)));
    }

    #[test]
    fn test_get_merged_config_item_project_not_found() {
        let storage = test_storage();
        let err = get_merged_config_item(&storage, "nope", "default", "k").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_shared_persistence() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        {
            let mut storage = Storage::load(&path).unwrap();
            setup_shared_env(&mut storage, "default");
            create_shared_item(
                &mut storage,
                "default",
                "log_level",
                serde_json::json!("info"),
            )
            .unwrap();
        }

        // 重新加载验证持久化
        let storage = Storage::load(&path).unwrap();
        let items = list_shared_items(&storage, "default").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].key, "log_level");
        assert_eq!(items[0].value, serde_json::json!("info"));
    }
}
