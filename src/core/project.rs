use crate::error::{ConfigError, Result};
use crate::models::{Environment, Project};
use crate::storage::Storage;

/// 创建项目，自动创建 "default" 环境。
/// 写时持久化：先修改内存，保存成功则完成，失败则回滚。
pub fn create_project(
    storage: &mut Storage,
    name: &str,
    description: Option<&str>,
) -> Result<Project> {
    // 检查名称唯一性
    if storage.state().projects.iter().any(|p| p.name == name) {
        return Err(ConfigError::ProjectAlreadyExists(name.to_string()));
    }

    let project = Project {
        name: name.to_string(),
        description: description.map(|d| d.to_string()),
        environments: vec![Environment {
            name: "default".to_string(),
            config_items: vec![],
        }],
    };

    // 写时持久化：先修改内存，尝试保存，失败则回滚
    storage.state_mut().projects.push(project.clone());

    if let Err(e) = storage.save() {
        // 回滚：移除刚添加的项目
        storage.state_mut().projects.pop();
        return Err(e);
    }

    Ok(project)
}

/// 列出所有项目
pub fn list_projects(storage: &Storage) -> Vec<&Project> {
    storage.state().projects.iter().collect()
}

/// 删除项目及其所有环境和配置项，同时删除绑定的 API Key。
/// 写时持久化：先修改内存，保存成功则完成，失败则回滚。
pub fn delete_project(storage: &mut Storage, name: &str) -> Result<()> {
    let state = storage.state();
    let pos = state
        .projects
        .iter()
        .position(|p| p.name == name)
        .ok_or_else(|| ConfigError::ProjectNotFound(name.to_string()))?;

    // 保存回滚数据
    let removed_project = storage.state_mut().projects.remove(pos);
    let removed_keys: Vec<_> = storage
        .state()
        .api_keys
        .iter()
        .enumerate()
        .filter(|(_, k)| k.project == name)
        .map(|(i, k)| (i, k.clone()))
        .collect();

    // 从后往前删除 API Key，避免索引偏移
    for (i, _) in removed_keys.iter().rev() {
        storage.state_mut().api_keys.remove(*i);
    }

    if let Err(e) = storage.save() {
        // 回滚：恢复项目和 API Key
        storage.state_mut().projects.insert(pos, removed_project);
        for (i, key) in removed_keys {
            storage.state_mut().api_keys.insert(i, key);
        }
        return Err(e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn test_storage() -> Storage {
        let tmp = NamedTempFile::new().unwrap();
        Storage::load(tmp.path()).unwrap()
    }

    #[test]
    fn test_create_project_basic() {
        let mut storage = test_storage();
        let project = create_project(&mut storage, "my-app", Some("desc")).unwrap();

        assert_eq!(project.name, "my-app");
        assert_eq!(project.description, Some("desc".to_string()));
        assert_eq!(project.environments.len(), 1);
        assert_eq!(project.environments[0].name, "default");
    }

    #[test]
    fn test_create_project_duplicate() {
        let mut storage = test_storage();
        create_project(&mut storage, "my-app", None).unwrap();

        let err = create_project(&mut storage, "my-app", None).unwrap_err();
        assert!(matches!(err, ConfigError::ProjectAlreadyExists(_)));
    }

    #[test]
    fn test_list_projects() {
        let mut storage = test_storage();
        assert!(list_projects(&storage).is_empty());

        create_project(&mut storage, "a", None).unwrap();
        create_project(&mut storage, "b", None).unwrap();

        let projects = list_projects(&storage);
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "a");
        assert_eq!(projects[1].name, "b");
    }

    #[test]
    fn test_delete_project() {
        let mut storage = test_storage();
        create_project(&mut storage, "my-app", None).unwrap();

        delete_project(&mut storage, "my-app").unwrap();
        assert!(list_projects(&storage).is_empty());
    }

    #[test]
    fn test_delete_project_not_found() {
        let mut storage = test_storage();
        let err = delete_project(&mut storage, "nope").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_delete_project_removes_api_keys() {
        let mut storage = test_storage();
        create_project(&mut storage, "my-app", None).unwrap();

        // 手动添加 API Key
        storage.state_mut().api_keys.push(crate::models::ApiKey {
            key: "key-1".to_string(),
            project: "my-app".to_string(),
        });
        storage.state_mut().api_keys.push(crate::models::ApiKey {
            key: "key-2".to_string(),
            project: "other".to_string(),
        });
        storage.save().unwrap();

        delete_project(&mut storage, "my-app").unwrap();

        assert!(list_projects(&storage).is_empty());
        assert_eq!(storage.state().api_keys.len(), 1);
        assert_eq!(storage.state().api_keys[0].project, "other");
    }

    #[test]
    fn test_create_project_default_env() {
        let mut storage = test_storage();
        let project = create_project(&mut storage, "test", None).unwrap();

        assert_eq!(project.environments.len(), 1);
        assert_eq!(project.environments[0].name, "default");
        assert!(project.environments[0].config_items.is_empty());
    }
}
