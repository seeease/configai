use std::collections::HashMap;
use std::path::Path;

use crate::error::{ConfigError, Result};
use crate::storage::Storage;

/// 配置中心：只读，从 YAML 目录加载
pub struct ConfigCenter {
    storage: Storage,
}

impl ConfigCenter {
    pub fn new(config_dir: &Path) -> Result<Self> {
        let storage = Storage::load(config_dir)?;
        Ok(Self { storage })
    }

    pub fn reload(&mut self, config_dir: &Path) -> Result<()> {
        self.storage = Storage::load(config_dir)?;
        Ok(())
    }

    pub fn list_projects(&self) -> Vec<&str> {
        self.storage.state().projects.keys().map(|s| s.as_str()).collect()
    }

    /// 合并配置：shared[env] 为底，project[env] 覆盖
    pub fn get_merged_config(
        &self,
        project: &str,
        env: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let state = self.storage.state();
        let proj = state
            .projects
            .get(project)
            .ok_or_else(|| ConfigError::ProjectNotFound(project.to_string()))?;

        let proj_env = proj
            .environments
            .get(env)
            .ok_or_else(|| ConfigError::EnvironmentNotFound(env.to_string()))?;

        let mut merged = HashMap::new();

        // shared 作为底层
        if let Some(shared_env) = state.shared.get(env) {
            merged.extend(shared_env.clone());
        }

        // 项目配置覆盖
        merged.extend(proj_env.clone());

        // 解析环境变量替换
        let resolved: HashMap<String, serde_json::Value> = merged
            .into_iter()
            .map(|(k, v)| (k, resolve_env_vars(v)))
            .collect();

        Ok(resolved)
    }

    pub fn get_merged_config_item(
        &self,
        project: &str,
        env: &str,
        key: &str,
    ) -> Result<serde_json::Value> {
        let merged = self.get_merged_config(project, env)?;
        merged
            .get(key)
            .cloned()
            .ok_or_else(|| ConfigError::ConfigItemNotFound(key.to_string()))
    }

    /// 验证 API Key，返回 (项目名, key)
    pub fn validate_api_key(&self, key: &str) -> Result<(&str, &str)> {
        let state = self.storage.state();
        for (project_name, project_data) in &state.projects {
            for api_key in &project_data.meta.api_keys {
                if api_key.key == key {
                    return Ok((project_name.as_str(), api_key.key.as_str()));
                }
            }
        }
        Err(ConfigError::Unauthorized("invalid api key".to_string()))
    }

    /// 将合并后的配置转换为环境变量 HashMap
    pub fn get_env_vars(
        &self,
        project: &str,
        env: &str,
        prefix: Option<&str>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let merged = self.get_merged_config(project, env)?;
        let mut vars = HashMap::new();

        for (key, value) in merged {
            let env_key = to_env_key(&key, prefix);
            vars.insert(env_key, value);
        }

        Ok(vars)
    }

    /// 生成 export 格式的字符串
    pub fn get_env_export(
        &self,
        project: &str,
        env: &str,
        prefix: Option<&str>,
    ) -> Result<String> {
        let vars = self.get_env_vars(project, env, prefix)?;
        let mut lines: Vec<String> = vars
            .iter()
            .map(|(k, v)| {
                let s = json_to_env_value(v);
                if needs_quoting(&s) {
                    format!("export {}=\"{}\"", k, s.replace('\\', "\\\\").replace('"', "\\\""))
                } else {
                    format!("export {}={}", k, s)
                }
            })
            .collect();
        lines.sort();
        Ok(lines.join("\n"))
    }
}

/// key 转环境变量名：大写，点和横线转下划线，加可选前缀
fn to_env_key(key: &str, prefix: Option<&str>) -> String {
    let normalized = key.replace('.', "_").replace('-', "_").to_uppercase();
    match prefix {
        Some(p) => format!("{}_{}", p.to_uppercase(), normalized),
        None => normalized,
    }
}

/// JSON 值转环境变量值
fn json_to_env_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        // 复杂类型序列化为 JSON 字符串
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// 判断值是否需要引号（包含空格或特殊字符）
fn needs_quoting(value: &str) -> bool {
    value.is_empty()
        || value.contains(' ')
        || value.contains('"')
        || value.contains('\'')
        || value.contains('$')
        || value.contains('`')
        || value.contains('\\')
        || value.contains('\n')
        || value.contains('{')
        || value.contains('}')
        || value.contains('[')
        || value.contains(']')
}
/// Recursively resolve ${VAR} patterns in JSON values using process environment variables.
/// - "${VAR}" as the entire string → replaced with env var value (string)
/// - "prefix_${VAR}_suffix" → string interpolation
/// - If env var is not set, keep the original "${VAR}" unchanged
fn resolve_env_vars(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(substitute_env_in_string(&s)),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(resolve_env_vars).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, resolve_env_vars(v)))
                .collect(),
        ),
        other => other, // numbers, bools, null unchanged
    }
}

/// Replace ${VAR} patterns in a string with environment variable values.
fn substitute_env_in_string(s: &str) -> String {
    let mut result = s.to_string();
    let mut search_from = 0;
    while let Some(rel_start) = result[search_from..].find("${") {
        let start = search_from + rel_start;
        if let Some(rel_end) = result[start..].find('}') {
            let end = start + rel_end;
            let var_name = &result[start + 2..end];
            match std::env::var(var_name) {
                Ok(val) => {
                    result = format!("{}{}{}", &result[..start], val, &result[end + 1..]);
                    search_from = start + val.len();
                }
                Err(_) => {
                    // 环境变量不存在，跳过这个 ${...}，继续往后搜
                    search_from = end + 1;
                }
            }
        } else {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// 辅助：创建临时配置目录结构
    fn setup_config_dir(tmp: &TempDir) {
        let base = tmp.path();
        std::fs::create_dir_all(base.join("shared")).unwrap();
        std::fs::create_dir_all(base.join("projects/my-app")).unwrap();

        // shared/default.yaml
        std::fs::write(
            base.join("shared/default.yaml"),
            "log_level: info\ntimeout: 30\n",
        )
        .unwrap();

        // projects/my-app/project.yaml
        std::fs::write(
            base.join("projects/my-app/project.yaml"),
            "description: \"测试项目\"\napi_keys:\n  - key: \"test-key-123\"\n",
        )
        .unwrap();

        // projects/my-app/default.yaml
        std::fs::write(
            base.join("projects/my-app/default.yaml"),
            "db_host: localhost\ndb_port: 5432\nlog_level: debug\n",
        )
        .unwrap();
    }

    #[test]
    fn test_load_and_list_projects() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let projects = center.list_projects();
        assert_eq!(projects.len(), 1);
        assert!(projects.contains(&"my-app"));
    }

    #[test]
    fn test_merged_config_project_overrides_shared() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let merged = center.get_merged_config("my-app", "default").unwrap();

        // 项目覆盖 shared 的 log_level
        assert_eq!(merged["log_level"], serde_json::json!("debug"));
        // shared 的 timeout 保留
        assert_eq!(merged["timeout"], serde_json::json!(30));
        // 项目独有
        assert_eq!(merged["db_host"], serde_json::json!("localhost"));
        assert_eq!(merged["db_port"], serde_json::json!(5432));
    }

    #[test]
    fn test_merged_config_item() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let val = center
            .get_merged_config_item("my-app", "default", "db_host")
            .unwrap();
        assert_eq!(val, serde_json::json!("localhost"));
    }

    #[test]
    fn test_merged_config_item_from_shared() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let val = center
            .get_merged_config_item("my-app", "default", "timeout")
            .unwrap();
        assert_eq!(val, serde_json::json!(30));
    }

    #[test]
    fn test_merged_config_item_not_found() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let err = center
            .get_merged_config_item("my-app", "default", "nonexistent")
            .unwrap_err();
        assert!(matches!(err, ConfigError::ConfigItemNotFound(_)));
    }

    #[test]
    fn test_project_not_found() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let err = center.get_merged_config("nope", "default").unwrap_err();
        assert!(matches!(err, ConfigError::ProjectNotFound(_)));
    }

    #[test]
    fn test_env_not_found() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let err = center.get_merged_config("my-app", "staging").unwrap_err();
        assert!(matches!(err, ConfigError::EnvironmentNotFound(_)));
    }

    #[test]
    fn test_validate_api_key_ok() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let (project, key) = center.validate_api_key("test-key-123").unwrap();
        assert_eq!(project, "my-app");
        assert_eq!(key, "test-key-123");
    }

    #[test]
    fn test_validate_api_key_invalid() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let err = center.validate_api_key("bad-key").unwrap_err();
        assert!(matches!(err, ConfigError::Unauthorized(_)));
    }

    #[test]
    fn test_env_vars_basic() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let vars = center.get_env_vars("my-app", "default", None).unwrap();

        assert_eq!(vars["DB_HOST"], serde_json::json!("localhost"));
        assert_eq!(vars["DB_PORT"], serde_json::json!(5432));
        assert_eq!(vars["LOG_LEVEL"], serde_json::json!("debug"));
        assert_eq!(vars["TIMEOUT"], serde_json::json!(30));
    }

    #[test]
    fn test_env_vars_with_prefix() {
        let tmp = TempDir::new().unwrap();
        setup_config_dir(&tmp);

        let center = ConfigCenter::new(tmp.path()).unwrap();
        let vars = center
            .get_env_vars("my-app", "default", Some("MY_APP"))
            .unwrap();

        assert_eq!(vars["MY_APP_DB_HOST"], serde_json::json!("localhost"));
        assert_eq!(vars["MY_APP_DB_PORT"], serde_json::json!(5432));
    }

    #[test]
    fn test_env_key_conversion() {
        assert_eq!(to_env_key("db_host", None), "DB_HOST");
        assert_eq!(to_env_key("redis.url", None), "REDIS_URL");
        assert_eq!(to_env_key("api-timeout", None), "API_TIMEOUT");
        assert_eq!(to_env_key("db_host", Some("APP")), "APP_DB_HOST");
    }

    #[test]
    fn test_json_to_env_value_types() {
        assert_eq!(json_to_env_value(&serde_json::json!("hello")), "hello");
        assert_eq!(json_to_env_value(&serde_json::json!(42)), "42");
        assert_eq!(json_to_env_value(&serde_json::json!(true)), "true");
        assert_eq!(json_to_env_value(&serde_json::json!(null)), "");
        // 复杂类型序列化为 JSON
        let arr = json_to_env_value(&serde_json::json!(["a", "b"]));
        assert_eq!(arr, r#"["a","b"]"#);
    }

    #[test]
    fn test_env_export_format() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(
            base.join("projects/app/default.yaml"),
            "db_host: localhost\ndb_port: 5432\n",
        )
        .unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let export = center.get_env_export("app", "default", None).unwrap();

        assert!(export.contains("export DB_HOST=localhost"));
        assert!(export.contains("export DB_PORT=5432"));
    }

    #[test]
    fn test_env_export_quoting() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(
            base.join("projects/app/default.yaml"),
            "greeting: hello world\n",
        )
        .unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let export = center.get_env_export("app", "default", None).unwrap();

        assert!(export.contains("export GREETING=\"hello world\""));
    }

    #[test]
    fn test_empty_config_dir() {
        let tmp = TempDir::new().unwrap();
        let center = ConfigCenter::new(tmp.path()).unwrap();
        assert!(center.list_projects().is_empty());
    }

    #[test]
    fn test_nonexistent_config_dir() {
        let center = ConfigCenter::new(Path::new("/tmp/nonexistent_config_dir_12345")).unwrap();
        assert!(center.list_projects().is_empty());
    }

    #[test]
    fn test_malformed_yaml_skipped() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        // 故意写入无效 YAML
        std::fs::write(base.join("projects/app/default.yaml"), "{{invalid yaml").unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let projects = center.list_projects();
        assert_eq!(projects.len(), 1);
        // 环境配置加载失败，应该没有 default 环境
        let state = center.storage.state();
        assert!(state.projects["app"].environments.is_empty());
    }

    #[test]
    fn test_reload() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(base.join("projects/app/default.yaml"), "port: 3000\n").unwrap();

        let mut center = ConfigCenter::new(base).unwrap();
        let merged = center.get_merged_config("app", "default").unwrap();
        assert_eq!(merged["port"], serde_json::json!(3000));

        // 修改文件
        std::fs::write(base.join("projects/app/default.yaml"), "port: 8080\n").unwrap();
        center.reload(base).unwrap();

        let merged = center.get_merged_config("app", "default").unwrap();
        assert_eq!(merged["port"], serde_json::json!(8080));
    }

    #[test]
    fn test_multiple_projects() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app1")).unwrap();
        std::fs::create_dir_all(base.join("projects/app2")).unwrap();
        std::fs::write(
            base.join("projects/app1/project.yaml"),
            "description: app1\napi_keys:\n  - key: key1\n",
        )
        .unwrap();
        std::fs::write(base.join("projects/app1/default.yaml"), "port: 3000\n").unwrap();
        std::fs::write(
            base.join("projects/app2/project.yaml"),
            "description: app2\napi_keys:\n  - key: key2\n",
        )
        .unwrap();
        std::fs::write(base.join("projects/app2/default.yaml"), "port: 4000\n").unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let mut projects = center.list_projects();
        projects.sort();
        assert_eq!(projects, vec!["app1", "app2"]);

        let (proj, _) = center.validate_api_key("key1").unwrap();
        assert_eq!(proj, "app1");
        let (proj, _) = center.validate_api_key("key2").unwrap();
        assert_eq!(proj, "app2");
    }

    #[test]
    fn test_multiple_environments() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("shared")).unwrap();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(base.join("projects/app/default.yaml"), "port: 3000\n").unwrap();
        std::fs::write(base.join("projects/app/production.yaml"), "port: 80\n").unwrap();
        std::fs::write(base.join("shared/default.yaml"), "log_level: info\n").unwrap();
        std::fs::write(base.join("shared/production.yaml"), "log_level: warn\n").unwrap();

        let center = ConfigCenter::new(base).unwrap();

        let default_cfg = center.get_merged_config("app", "default").unwrap();
        assert_eq!(default_cfg["port"], serde_json::json!(3000));
        assert_eq!(default_cfg["log_level"], serde_json::json!("info"));

        let prod_cfg = center.get_merged_config("app", "production").unwrap();
        assert_eq!(prod_cfg["port"], serde_json::json!(80));
        assert_eq!(prod_cfg["log_level"], serde_json::json!("warn"));
    }

    #[test]
    fn test_complex_yaml_values() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(
            base.join("projects/app/default.yaml"),
            "hosts:\n  - a\n  - b\ndb:\n  host: localhost\n  port: 5432\nenabled: true\ncount: 42\n",
        )
        .unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let merged = center.get_merged_config("app", "default").unwrap();

        assert_eq!(merged["hosts"], serde_json::json!(["a", "b"]));
        assert_eq!(
            merged["db"],
            serde_json::json!({"host": "localhost", "port": 5432})
        );
        assert_eq!(merged["enabled"], serde_json::json!(true));
        assert_eq!(merged["count"], serde_json::json!(42));
    }
    #[test]
    fn test_env_var_substitution() {
        std::env::set_var("TEST_DB_PASSWORD", "secret123");
        std::env::set_var("TEST_JWT_SECRET", "jwt-key");

        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(
            base.join("projects/app/default.yaml"),
            "db_password: \"${TEST_DB_PASSWORD}\"\njwt_secret: \"${TEST_JWT_SECRET}\"\ndb_host: localhost\n",
        )
        .unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let merged = center.get_merged_config("app", "default").unwrap();

        assert_eq!(merged["db_password"], serde_json::json!("secret123"));
        assert_eq!(merged["jwt_secret"], serde_json::json!("jwt-key"));
        assert_eq!(merged["db_host"], serde_json::json!("localhost"));

        std::env::remove_var("TEST_DB_PASSWORD");
        std::env::remove_var("TEST_JWT_SECRET");
    }

    #[test]
    fn test_env_var_substitution_missing_var() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(
            base.join("projects/app/default.yaml"),
            "secret: \"${NONEXISTENT_VAR_12345}\"\n",
        )
        .unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let merged = center.get_merged_config("app", "default").unwrap();

        assert_eq!(
            merged["secret"],
            serde_json::json!("${NONEXISTENT_VAR_12345}")
        );
    }

    #[test]
    fn test_env_var_substitution_in_nested() {
        std::env::set_var("TEST_NESTED_KEY", "resolved-value");

        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(
            base.join("projects/app/default.yaml"),
            "db:\n  password: \"${TEST_NESTED_KEY}\"\nlist:\n  - \"${TEST_NESTED_KEY}\"\n  - plain\n",
        )
        .unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let merged = center.get_merged_config("app", "default").unwrap();

        assert_eq!(
            merged["db"]["password"],
            serde_json::json!("resolved-value")
        );
        assert_eq!(merged["list"][0], serde_json::json!("resolved-value"));
        assert_eq!(merged["list"][1], serde_json::json!("plain"));

        std::env::remove_var("TEST_NESTED_KEY");
    }

    #[test]
    fn test_env_var_inline_substitution() {
        std::env::set_var("TEST_HOST", "db.example.com");
        std::env::set_var("TEST_PORT", "5432");

        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("projects/app")).unwrap();
        std::fs::write(
            base.join("projects/app/project.yaml"),
            "api_keys:\n  - key: k\n",
        )
        .unwrap();
        std::fs::write(
            base.join("projects/app/default.yaml"),
            "db_url: \"postgres://user@${TEST_HOST}:${TEST_PORT}/mydb\"\n",
        )
        .unwrap();

        let center = ConfigCenter::new(base).unwrap();
        let merged = center.get_merged_config("app", "default").unwrap();

        assert_eq!(
            merged["db_url"],
            serde_json::json!("postgres://user@db.example.com:5432/mydb")
        );

        std::env::remove_var("TEST_HOST");
        std::env::remove_var("TEST_PORT");
    }

    #[test]
    fn test_substitute_env_in_string() {
        std::env::set_var("TEST_SUB_A", "hello");
        assert_eq!(substitute_env_in_string("${TEST_SUB_A}"), "hello");
        assert_eq!(
            substitute_env_in_string("prefix_${TEST_SUB_A}_suffix"),
            "prefix_hello_suffix"
        );
        assert_eq!(substitute_env_in_string("no vars here"), "no vars here");
        assert_eq!(
            substitute_env_in_string("${MISSING_VAR_XYZ}"),
            "${MISSING_VAR_XYZ}"
        );
        std::env::remove_var("TEST_SUB_A");
    }
}
