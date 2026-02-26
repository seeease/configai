use crate::error::{ConfigError, Result};
use crate::models::ConfigItem;
use crate::storage::Storage;

/// 在指定项目和环境下创建配置项。
/// 检查项目存在、环境存在、键唯一性。写时持久化，失败回滚。
pub fn create_config_item(
    storage: &mut Storage,
    project: &str,
    env: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<ConfigItem> {
    // 验证项目存在
    let proj = storage
        .state()
        .projects
        .iter()
        .find(|p| p.name == project)
        .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

    // 验证环境存在
    let environment = proj
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
    let environment = storage
        .state_mut()
        .projects
        .iter_mut()
        .find(|p| p.name == project)
        .unwrap()
        .environments
        .iter_mut()
        .find(|e| e.name == env)
        .unwrap();
    environment.config_items.push(config_item.clone());

    if let Err(e) = storage.save() {
        // 回滚
        let environment = storage
            .state_mut()
            .projects
            .iter_mut()
            .find(|p| p.name == project)
            .unwrap()
            .environments
            .iter_mut()
            .find(|e| e.name == env)
            .unwrap();
        environment.config_items.pop();
        return Err(e);
    }

    Ok(config_item)
}


/// 更新配置项值。验证项目、环境和键存在。写时持久化，失败回滚。
pub fn update_config_item(
    storage: &mut Storage,
    project: &str,
    env: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<ConfigItem> {
    // 验证项目存在
    let proj = storage
        .state()
        .projects
        .iter()
        .find(|p| p.name == project)
        .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

    // 验证环境存在
    let environment = proj
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
        .projects
        .iter_mut()
        .find(|p| p.name == project)
        .unwrap()
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
        // 回滚
        let item = storage
            .state_mut()
            .projects
            .iter_mut()
            .find(|p| p.name == project)
            .unwrap()
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

/// 删除配置项。写时持久化，失败回滚。
pub fn delete_config_item(
    storage: &mut Storage,
    project: &str,
    env: &str,
    key: &str,
) -> Result<()> {
    // 验证项目存在
    let proj = storage
        .state()
        .projects
        .iter()
        .find(|p| p.name == project)
        .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

    // 验证环境存在
    let environment = proj
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
        .projects
        .iter_mut()
        .find(|p| p.name == project)
        .unwrap()
        .environments
        .iter_mut()
        .find(|e| e.name == env)
        .unwrap()
        .config_items
        .remove(pos);

    if let Err(e) = storage.save() {
        // 回滚
        storage
            .state_mut()
            .projects
            .iter_mut()
            .find(|p| p.name == project)
            .unwrap()
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

/// 列出指定项目和环境下的所有配置项
pub fn list_config_items<'a>(
    storage: &'a Storage,
    project: &str,
    env: &str,
) -> Result<Vec<&'a ConfigItem>> {
    // 验证项目存在
    let proj = storage
        .state()
        .projects
        .iter()
        .find(|p| p.name == project)
        .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

    // 验证环境存在
    let environment = proj
        .environments
        .iter()
        .find(|e| e.name == env)
        .ok_or_else(|| ConfigError::EnvironmentNotFound(env.to_string()))?;

    Ok(environment.config_items.iter().collect())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::env::create_environment;
    use crate::core::project::create_project;
    use tempfile::NamedTempFile;

    fn test_storage() -> Storage {
        let tmp = NamedTempFile::new().unwrap();
        Storage::load(tmp.path()).unwrap()
    }

    /// 辅助：创建项目 + 环境，返回 storage
    fn setup_project_env(storage: &mut Storage, project: &str, env: &str) {
        create_project(storage, project, None).unwrap();
        if env != "default" {
            create_environment(storage, project, env).unwrap();
        }
    }

    #[test]
    fn test_create_config_item_string() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");

        let item = create_config_item(
            &mut storage,
            "app",
            "default",
            "db_host",
            serde_json::json!("localhost"),
        )
        .unwrap();

        assert_eq!(item.key, "db_host");
        assert_eq!(item.value, serde_json::json!("localhost"));
    }

    #[test]
    fn test_create_config_item_various_json_types() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");

        // number
        let item = create_config_item(&mut storage, "app", "default", "port", serde_json::json!(8080)).unwrap();
        assert_eq!(item.value, serde_json::json!(8080));

        // boolean
        let item = create_config_item(&mut storage, "app", "default", "debug", serde_json::json!(false)).unwrap();
        assert_eq!(item.value, serde_json::json!(false));

        // null
        let item = create_config_item(&mut storage, "app", "default", "optional", serde_json::json!(null)).unwrap();
        assert_eq!(item.value, serde_json::json!(null));

        // array
        let item = create_config_item(&mut storage, "app", "default", "hosts", serde_json::json!(["a", "b"])).unwrap();
        assert_eq!(item.value, serde_json::json!(["a", "b"]));

        // object
        let item = create_config_item(&mut storage, "app", "default", "db", serde_json::json!({"host": "localhost", "port": 5432})).unwrap();
        assert_eq!(item.value, serde_json::json!({"host": "localhost", "port": 5432}));
    }

    #[test]
    fn test_create_config_item_duplicate_key() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");

        create_config_item(&mut storage, "app", "default", "key1", serde_json::json!("v1")).unwrap();
        let err = create_config_item(&mut storage, "app", "default", "key1", serde_json::json!("v2")).unwrap_err();
        assert!(matches!(err, ConfigError::ConfigItemAlreadyExists(_)));
    }

    #[test]
    fn test_create_config_item_project_not_found() {
        let mut storage = test_storage();
        let err = create_config_item(&mut storage, "nope", "default", "k", serde_json::json!("v")).unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_create_config_item_env_not_found() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        let err = create_config_item(&mut storage, "app", "nope", "k", serde_json::json!("v")).unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_update_config_item() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");
        create_config_item(&mut storage, "app", "default", "key1", serde_json::json!("old")).unwrap();

        let updated = update_config_item(&mut storage, "app", "default", "key1", serde_json::json!(42)).unwrap();
        assert_eq!(updated.key, "key1");
        assert_eq!(updated.value, serde_json::json!(42));

        // 验证列表也反映了更新
        let items = list_config_items(&storage, "app", "default").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].value, serde_json::json!(42));
    }

    #[test]
    fn test_update_config_item_not_found() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");

        let err = update_config_item(&mut storage, "app", "default", "nope", serde_json::json!("v")).unwrap_err();
        assert!(matches!(err, ConfigError::ConfigItemNotFound(_)));
    }

    #[test]
    fn test_update_config_item_project_not_found() {
        let mut storage = test_storage();
        let err = update_config_item(&mut storage, "nope", "default", "k", serde_json::json!("v")).unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_update_config_item_env_not_found() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        let err = update_config_item(&mut storage, "app", "nope", "k", serde_json::json!("v")).unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_delete_config_item() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");
        create_config_item(&mut storage, "app", "default", "key1", serde_json::json!("v")).unwrap();

        delete_config_item(&mut storage, "app", "default", "key1").unwrap();

        let items = list_config_items(&storage, "app", "default").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_delete_config_item_not_found() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");

        let err = delete_config_item(&mut storage, "app", "default", "nope").unwrap_err();
        assert!(matches!(err, ConfigError::ConfigItemNotFound(_)));
    }

    #[test]
    fn test_delete_config_item_project_not_found() {
        let mut storage = test_storage();
        let err = delete_config_item(&mut storage, "nope", "default", "k").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_delete_config_item_env_not_found() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        let err = delete_config_item(&mut storage, "app", "nope", "k").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_list_config_items() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");

        assert!(list_config_items(&storage, "app", "default").unwrap().is_empty());

        create_config_item(&mut storage, "app", "default", "k1", serde_json::json!("v1")).unwrap();
        create_config_item(&mut storage, "app", "default", "k2", serde_json::json!(2)).unwrap();

        let items = list_config_items(&storage, "app", "default").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].key, "k1");
        assert_eq!(items[1].key, "k2");
    }

    #[test]
    fn test_list_config_items_project_not_found() {
        let storage = test_storage();
        let err = list_config_items(&storage, "nope", "default").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_list_config_items_env_not_found() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        let err = list_config_items(&storage, "app", "nope").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_persistence() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        {
            let mut storage = Storage::load(&path).unwrap();
            setup_project_env(&mut storage, "app", "default");
            create_config_item(&mut storage, "app", "default", "key1", serde_json::json!({"nested": true})).unwrap();
        }

        // 重新加载验证持久化
        let storage = Storage::load(&path).unwrap();
        let items = list_config_items(&storage, "app", "default").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].key, "key1");
        assert_eq!(items[0].value, serde_json::json!({"nested": true}));
    }

    #[test]
    fn test_json_type_preserved_through_update() {
        let mut storage = test_storage();
        setup_project_env(&mut storage, "app", "default");

        // 创建字符串，更新为数组
        create_config_item(&mut storage, "app", "default", "k", serde_json::json!("str")).unwrap();
        let updated = update_config_item(&mut storage, "app", "default", "k", serde_json::json!([1, 2, 3])).unwrap();
        assert!(updated.value.is_array());
        assert_eq!(updated.value, serde_json::json!([1, 2, 3]));
    }
}
