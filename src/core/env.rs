use crate::error::{ConfigError, Result};
use crate::models::Environment;
use crate::storage::Storage;

/// 在项目下创建环境，检查名称唯一性。
/// 写时持久化：先修改内存，保存成功则完成，失败则回滚。
pub fn create_environment(
    storage: &mut Storage,
    project: &str,
    env_name: &str,
) -> Result<Environment> {
    let proj = storage
        .state()
        .projects
        .iter()
        .find(|p| p.name == project)
        .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

    // 检查环境名称唯一性
    if proj.environments.iter().any(|e| e.name == env_name) {
        return Err(ConfigError::EnvironmentAlreadyExists(env_name.to_string()));
    }

    let env = Environment {
        name: env_name.to_string(),
        config_items: vec![],
    };

    // 写时持久化：先修改内存，尝试保存，失败则回滚
    let proj = storage
        .state_mut()
        .projects
        .iter_mut()
        .find(|p| p.name == project)
        .unwrap();
    proj.environments.push(env.clone());

    if let Err(e) = storage.save() {
        // 回滚：移除刚添加的环境
        let proj = storage
            .state_mut()
            .projects
            .iter_mut()
            .find(|p| p.name == project)
            .unwrap();
        proj.environments.pop();
        return Err(e);
    }

    Ok(env)
}

/// 列出项目下所有环境
pub fn list_environments<'a>(storage: &'a Storage, project: &str) -> Result<Vec<&'a Environment>> {
    let proj = storage
        .state()
        .projects
        .iter()
        .find(|p| p.name == project)
        .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

    Ok(proj.environments.iter().collect())
}

/// 删除环境及其所有配置项。
/// 写时持久化：先修改内存，保存成功则完成，失败则回滚。
pub fn delete_environment(
    storage: &mut Storage,
    project: &str,
    env_name: &str,
) -> Result<()> {
    let proj = storage
        .state()
        .projects
        .iter()
        .find(|p| p.name == project)
        .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

    let env_pos = proj
        .environments
        .iter()
        .position(|e| e.name == env_name)
        .ok_or_else(|| ConfigError::EnvironmentNotFound(env_name.to_string()))?;

    // 写时持久化：先修改内存，尝试保存，失败则回滚
    let proj = storage
        .state_mut()
        .projects
        .iter_mut()
        .find(|p| p.name == project)
        .unwrap();
    let removed_env = proj.environments.remove(env_pos);

    if let Err(e) = storage.save() {
        // 回滚：恢复环境
        let proj = storage
            .state_mut()
            .projects
            .iter_mut()
            .find(|p| p.name == project)
            .unwrap();
        proj.environments.insert(env_pos, removed_env);
        return Err(e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::project::create_project;
    use tempfile::NamedTempFile;

    fn test_storage() -> Storage {
        let tmp = NamedTempFile::new().unwrap();
        Storage::load(tmp.path()).unwrap()
    }

    #[test]
    fn test_create_environment() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let env = create_environment(&mut storage, "app", "staging").unwrap();
        assert_eq!(env.name, "staging");
        assert!(env.config_items.is_empty());

        let envs = list_environments(&storage, "app").unwrap();
        assert_eq!(envs.len(), 2); // default + staging
        assert_eq!(envs[1].name, "staging");
    }

    #[test]
    fn test_create_environment_duplicate() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        create_environment(&mut storage, "app", "prod").unwrap();
        let err = create_environment(&mut storage, "app", "prod").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentAlreadyExists(_)));
    }

    #[test]
    fn test_create_environment_duplicate_default() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        // "default" 已由 create_project 自动创建
        let err = create_environment(&mut storage, "app", "default").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentAlreadyExists(_)));
    }

    #[test]
    fn test_create_environment_project_not_found() {
        let mut storage = test_storage();
        let err = create_environment(&mut storage, "nope", "dev").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_list_environments() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let envs = list_environments(&storage, "app").unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "default");

        create_environment(&mut storage, "app", "dev").unwrap();
        create_environment(&mut storage, "app", "prod").unwrap();

        let envs = list_environments(&storage, "app").unwrap();
        assert_eq!(envs.len(), 3);
    }

    #[test]
    fn test_list_environments_project_not_found() {
        let storage = test_storage();
        let err = list_environments(&storage, "nope").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_delete_environment() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        create_environment(&mut storage, "app", "staging").unwrap();

        delete_environment(&mut storage, "app", "staging").unwrap();

        let envs = list_environments(&storage, "app").unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "default");
    }

    #[test]
    fn test_delete_environment_with_config_items() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();
        create_environment(&mut storage, "app", "dev").unwrap();

        // 手动添加配置项
        let proj = storage
            .state_mut()
            .projects
            .iter_mut()
            .find(|p| p.name == "app")
            .unwrap();
        let env = proj.environments.iter_mut().find(|e| e.name == "dev").unwrap();
        env.config_items.push(crate::models::ConfigItem {
            key: "key1".to_string(),
            value: serde_json::json!("value1"),
        });
        env.config_items.push(crate::models::ConfigItem {
            key: "key2".to_string(),
            value: serde_json::json!(42),
        });
        storage.save().unwrap();

        // 删除环境应级联删除所有配置项
        delete_environment(&mut storage, "app", "dev").unwrap();

        let envs = list_environments(&storage, "app").unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "default");
    }

    #[test]
    fn test_delete_environment_not_found() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let err = delete_environment(&mut storage, "app", "nope").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_delete_environment_project_not_found() {
        let mut storage = test_storage();
        let err = delete_environment(&mut storage, "nope", "dev").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_persistence_after_create() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        {
            let mut storage = Storage::load(&path).unwrap();
            create_project(&mut storage, "app", None).unwrap();
            create_environment(&mut storage, "app", "prod").unwrap();
        }

        // 重新加载，验证持久化
        let storage = Storage::load(&path).unwrap();
        let envs = list_environments(&storage, "app").unwrap();
        assert_eq!(envs.len(), 2);
        assert_eq!(envs[0].name, "default");
        assert_eq!(envs[1].name, "prod");
    }

    #[test]
    fn test_persistence_after_delete() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        {
            let mut storage = Storage::load(&path).unwrap();
            create_project(&mut storage, "app", None).unwrap();
            create_environment(&mut storage, "app", "staging").unwrap();
            delete_environment(&mut storage, "app", "staging").unwrap();
        }

        // 重新加载，验证持久化
        let storage = Storage::load(&path).unwrap();
        let envs = list_environments(&storage, "app").unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "default");
    }
}
