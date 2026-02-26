# configai

轻量级配置中心服务，Rust 编写。类似 nginx 的 conf.d 模式，用户直接编辑 YAML 文件管理配置，通过 REST API 对外提供配置读取服务。

## 快速开始

```bash
# 构建
cargo build --release

# 初始化配置目录（生成示例文件）
cargo run -- init

# 启动 API Server（默认端口 3000）
cargo run -- serve
```

## 配置目录结构

```
config/
├── shared/                    # 公共配置（所有项目共享）
│   ├── default.yaml
│   └── prod.yaml
├── projects/
│   ├── my-app/
│   │   ├── project.yaml       # 项目元信息 + API Keys
│   │   ├── default.yaml       # default 环境配置
│   │   └── prod.yaml          # prod 环境配置
│   └── another-app/
│       ├── project.yaml
│       └── default.yaml
```

规则：
- `projects/` 下每个子目录是一个项目，目录名即项目名
- `project.yaml` 存放项目描述和 API Keys
- 其他 `*.yaml` 文件是环境配置，文件名即环境名
- `shared/` 下的 YAML 文件是公共配置，文件名即环境名
- 合并逻辑：shared 配置为底层，项目配置覆盖同名 key

## 配置文件示例

`config/projects/my-app/project.yaml`:
```yaml
description: "我的应用"
api_keys:
  - key: "550e8400-e29b-41d4-a716-446655440000"
```

`config/projects/my-app/prod.yaml`:
```yaml
db_host: localhost
db_port: 5432
api_keys:
  - provider: openai
    key: sk-xxx
    weight: 3
```

`config/shared/prod.yaml`:
```yaml
log_level: info
retry_config:
  max_retries: 3
  backoff_ms: 1000
```

## 命令行参数

```bash
# 默认启动 API Server
cargo run -- serve

# 指定配置目录和端口
cargo run -- serve --config-dir /etc/configai --port 8080

# 初始化配置目录
cargo run -- init --config-dir ./my-config
```

## REST API

认证方式：`X-API-Key` 请求头，API Key 在 `project.yaml` 中配置。

### 获取合并后的全部配置

```bash
curl -H "X-API-Key: YOUR_API_KEY" \
  http://localhost:3000/api/v1/projects/my-app/envs/prod/configs
```

响应（包含原始配置和环境变量映射）：
```json
{
  "project": "my-app",
  "environment": "prod",
  "configs": {
    "db_host": "localhost",
    "db_port": 5432,
    "log_level": "info"
  },
  "env_vars": {
    "DB_HOST": "localhost",
    "DB_PORT": "5432",
    "LOG_LEVEL": "info"
  }
}
```

### 获取单个配置项

```bash
curl -H "X-API-Key: YOUR_API_KEY" \
  http://localhost:3000/api/v1/projects/my-app/envs/prod/configs/db_host
```

### 导出为环境变量

```bash
# 导出为 shell 环境变量格式
curl -H "X-API-Key: YOUR_API_KEY" \
  http://localhost:3000/api/v1/projects/my-app/envs/prod/export

# 带前缀
curl -H "X-API-Key: YOUR_API_KEY" \
  "http://localhost:3000/api/v1/projects/my-app/envs/prod/export?prefix=MY_APP"

# 直接注入环境变量
source <(curl -s -H "X-API-Key: YOUR_API_KEY" \
  http://localhost:3000/api/v1/projects/my-app/envs/prod/export)
```

输出：
```
export DB_HOST=localhost
export DB_PORT=5432
export LOG_LEVEL=info
```

### 环境变量转换规则

| YAML key | 环境变量 | 说明 |
|----------|---------|------|
| `db_host` | `DB_HOST` | 小写转大写 |
| `redis.url` | `REDIS_URL` | 点号转下划线 |
| `api-timeout` | `API_TIMEOUT` | 横线转下划线 |
| 复杂值 | JSON 字符串 | 对象/数组序列化为 JSON |

### 错误响应

- 缺少或无效 API Key → 401
- API Key 与请求项目不匹配 → 403
- 项目/环境/配置项不存在 → 404

## 热加载

API Server 通过 `notify` 监听配置目录变化，编辑 YAML 文件后自动重新加载，无需重启服务。

## 测试

```bash
cargo test
```
