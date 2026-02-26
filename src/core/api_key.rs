use uuid::Uuid;

use crate::error::{ConfigError, Result};
use crate::models::ApiKey;
use crate::storage::Storage;

/// 生成 API Key，绑定到指定项目。
/// 验证项目存在，生成 UUID v4，写时持久化，失败回滚。
pub fn generate_api_key(storage: &mut Storage, project: &str) -> Result<ApiKey> {
    // 验证项目存在
    if !storage.state().projects.iter().any(|p| p.name == project) {
        return Err(ConfigError::ProjectNotFound(project.to_string()));
    }

    let api_key = ApiKey {
        key: Uuid::new_v4().to_string(),
        project: project.to_string(),
    };

    // 写时持久化
    storage.state_mut().api_keys.push(api_key.clone());

    if let Err(e) = storage.save() {
        storage.state_mut().api_keys.pop();
        return Err(e);
    }

    Ok(api_key)
}

/// 撤销 API Key。
/// 写时持久化，失败回滚。
pub fn revoke_api_key(storage: &mut Storage, key: &str) -> Result<()> {
    let pos = storage
        .state()
        .api_keys
        .iter()
        .position(|k| k.key == key)
        .ok_or_else(|| ConfigError::ApiKeyNotFound(key.to_string()))?;

    let removed = storage.state_mut().api_keys.remove(pos);

    if let Err(e) = storage.save() {
        storage.state_mut().api_keys.insert(pos, removed);
        return Err(e);
    }

    Ok(())
}

/// 列出项目下所有 API Key。
/// 验证项目存在。
pub fn list_api_keys<'a>(storage: &'a Storage, project: &str) -> Result<Vec<&'a ApiKey>> {
    // 验证项目存在
    if !storage.state().projects.iter().any(|p| p.name == project) {
        return Err(ConfigError::ProjectNotFound(project.to_string()));
    }

    Ok(storage
        .state()
        .api_keys
        .iter()
        .filter(|k| k.project == project)
        .collect())
}

/// 验证 API Key 有效性。
/// 返回 ApiKeyNotFound 如果 key 不存在。
pub fn validate_api_key<'a>(storage: &'a Storage, key: &str) -> Result<&'a ApiKey> {
    storage
        .state()
        .api_keys
        .iter()
        .find(|k| k.key == key)
        .ok_or_else(|| ConfigError::ApiKeyNotFound(key.to_string()))
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
    fn test_generate_api_key() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let key = generate_api_key(&mut storage, "app").unwrap();
        assert_eq!(key.project, "app");
        assert!(!key.key.is_empty());
        // UUID v4 格式: 8-4-4-4-12
        assert_eq!(key.key.len(), 36);
    }

    #[test]
    fn test_generate_api_key_project_not_found() {
        let mut storage = test_storage();
        let err = generate_api_key(&mut storage, "nope").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_generate_multiple_keys() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let k1 = generate_api_key(&mut storage, "app").unwrap();
        let k2 = generate_api_key(&mut storage, "app").unwrap();
        assert_ne!(k1.key, k2.key);

        let keys = list_api_keys(&storage, "app").unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_revoke_api_key() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let key = generate_api_key(&mut storage, "app").unwrap();
        revoke_api_key(&mut storage, &key.key).unwrap();

        let keys = list_api_keys(&storage, "app").unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_revoke_api_key_not_found() {
        let mut storage = test_storage();
        let err = revoke_api_key(&mut storage, "nonexistent").unwrap_err();
        assert!(matches!(err, ConfigError::ApiKeyNotFound(_)));
    }

    #[test]
    fn test_list_api_keys_empty() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let keys = list_api_keys(&storage, "app").unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_list_api_keys_filters_by_project() {
        let mut storage = test_storage();
        create_project(&mut storage, "app1", None).unwrap();
        create_project(&mut storage, "app2", None).unwrap();

        generate_api_key(&mut storage, "app1").unwrap();
        generate_api_key(&mut storage, "app1").unwrap();
        generate_api_key(&mut storage, "app2").unwrap();

        let keys1 = list_api_keys(&storage, "app1").unwrap();
        assert_eq!(keys1.len(), 2);
        assert!(keys1.iter().all(|k| k.project == "app1"));

        let keys2 = list_api_keys(&storage, "app2").unwrap();
        assert_eq!(keys2.len(), 1);
        assert_eq!(keys2[0].project, "app2");
    }

    #[test]
    fn test_list_api_keys_project_not_found() {
        let storage = test_storage();
        let err = list_api_keys(&storage, "nope").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_validate_api_key() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let key = generate_api_key(&mut storage, "app").unwrap();
        let validated = validate_api_key(&storage, &key.key).unwrap();
        assert_eq!(validated.key, key.key);
        assert_eq!(validated.project, "app");
    }

    #[test]
    fn test_validate_api_key_not_found() {
        let storage = test_storage();
        let err = validate_api_key(&storage, "invalid-key").unwrap_err();
        assert!(matches!(err, ConfigError::ApiKeyNotFound(_)));
    }

    #[test]
    fn test_validate_revoked_key() {
        let mut storage = test_storage();
        create_project(&mut storage, "app", None).unwrap();

        let key = generate_api_key(&mut storage, "app").unwrap();
        revoke_api_key(&mut storage, &key.key).unwrap();

        let err = validate_api_key(&storage, &key.key).unwrap_err();
        assert!(matches!(err, ConfigError::ApiKeyNotFound(_)));
    }

    #[test]
    fn test_persistence() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let key_str;
        {
            let mut storage = Storage::load(&path).unwrap();
            create_project(&mut storage, "app", None).unwrap();
            let key = generate_api_key(&mut storage, "app").unwrap();
            key_str = key.key.clone();
        }

        // 重新加载验证持久化
        let storage = Storage::load(&path).unwrap();
        let validated = validate_api_key(&storage, &key_str).unwrap();
        assert_eq!(validated.project, "app");
    }
}
