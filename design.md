# configai

> 轻量级配置中心服务：nginx conf.d 风格 YAML 目录配置 + REST API 读取 + 环境变量导出，自动热加载，无外部数据库依赖。

## 技术栈

- 语言: Rust (edition 2021)
- HTTP: axum 0.8
- 序列化: serde + serde_json + serde_yaml
- 异步运行时: tokio
- 日志: tracing + tracing-subscriber
- 错误处理: thiserror
- API Key: uuid v4
- 文件监听: notify 7
- 测试: proptest, tempfile

## 架构

```
src/
├── main.rs              # 入口，init/serve 子命令
├── models.rs            # ConfigState, ProjectData, ProjectMeta, ApiKeyEntry
├── error.rs             # ConfigError 枚举
├── core/
│   └── mod.rs           # ConfigCenter（只读门面）
├── storage/
│   ├── mod.rs
│   └── dir.rs           # YAML 目录扫描存储引擎
├── api/
│   ├── mod.rs
│   ├── routes.rs        # GET 路由（configs + export）
│   ├── handlers.rs      # 请求处理器 + 响应类型
│   └── auth.rs          # X-API-Key 中间件
```

## 配置目录结构

```
config/
├── shared/{env}.yaml              # 公共配置
├── projects/{name}/project.yaml   # 项目元信息 + API Keys
├── projects/{name}/{env}.yaml     # 项目环境配置
```

文件名（不含 .yaml）即环境名，目录名即项目名。

## 数据模型

- ConfigState: projects (HashMap<String, ProjectData>), shared (HashMap<String, HashMap<String, Value>>)
- ProjectData: meta (ProjectMeta), environments (HashMap<String, HashMap<String, Value>>)
- ProjectMeta: description (Option<String>), api_keys (Vec<ApiKeyEntry>)
- ApiKeyEntry: key (String)

## ConfigCenter 公共 API

### 构造与重载
- new(config_dir: &Path) -> Result<Self>
- reload(config_dir: &Path) -> Result<()>

### 查询
- list_projects() -> Vec<&str>
- get_merged_config(project, env) -> Result<HashMap<String, Value>>
- get_merged_config_item(project, env, key) -> Result<Value>
- validate_api_key(key) -> Result<(&str, &str)> // (project_name, key)

### 环境变量
- get_env_vars(project, env, prefix: Option) -> Result<HashMap<String, String>>
- get_env_export(project, env, prefix: Option) -> Result<String>

合并逻辑: shared[env] 为基础，project[env] 覆盖同名 key。

## 环境变量转换规则

- 小写转大写: db_host → DB_HOST
- 点号转下划线: redis.url → REDIS_URL
- 横线转下划线: api-timeout → API_TIMEOUT
- 可选前缀: prefix=MY_APP → MY_APP_DB_HOST
- 复杂值（对象/数组）序列化为 JSON 字符串
- 包含空格等特殊字符的值自动加引号

## REST API

### 端点
- GET /api/v1/projects/{project}/envs/{env}/configs — 合并配置 + env_vars
- GET /api/v1/projects/{project}/envs/{env}/configs/{key} — 单个配置项
- GET /api/v1/projects/{project}/envs/{env}/export?prefix=XXX — shell export 格式

### 认证
请求头: X-API-Key（在 project.yaml 中配置）
- 缺少 → 401
- 无效 → 401
- 项目不匹配 → 403

### 响应格式
全部配置: {"project", "environment", "configs": {...}, "env_vars": {...}}
单个配置: {"key", "value"}
导出: export KEY=value\nexport KEY2=value2
错误: {"error": "..."}

## 错误类型

| 变体 | HTTP |
|------|------|
| ProjectNotFound | 404 |
| EnvironmentNotFound | 404 |
| ConfigItemNotFound | 404 |
| Unauthorized | 401 |
| Forbidden | 403 |
| StorageError | 500 |
| IoError | 500 |

## 运行模式

- init: 初始化配置目录结构（含示例文件）
- serve（默认）: 启动 API Server，递归监听配置目录变化自动 reload

参数: --config-dir (默认 ./config), --port (默认 3000)

## 构建与测试

```
cargo build --release
cargo run -- init
cargo run -- serve
cargo run -- serve --config-dir /etc/myconfig --port 8080
cargo test
```
