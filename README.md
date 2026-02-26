# config-center

轻量级配置中心服务，使用 Rust 编写。提供 TUI 终端管理界面和 REST API 配置读取接口。数据存储采用内存 + JSON 文件持久化，无需外部数据库依赖。

## 构建与运行

```bash
# 构建
cargo build --release

# 运行（启动 TUI 管理界面）
cargo run
```

运行后进入 TUI 界面，REST API 服务可在 TUI 中通过 Server 面板启停。

## TUI 使用

### 快捷键

| 按键 | 功能 |
|------|------|
| `q` | 退出 |
| `Tab` | 切换焦点（菜单 ↔ 内容） |
| `↑↓` | 导航 |
| `n` | 新建 / 生成 API Key |
| `d` | 删除 / 吊销 |
| `e` | 编辑配置项 |
| `p` | 切换项目上下文 |
| `v` | 切换环境上下文 |
| `s` | 启停 API 服务（Server 面板） |

### 典型工作流

1. 启动后在 Projects 面板按 `n` 创建项目
2. 切换到 Environments 面板管理环境（创建项目时自动创建 `default` 环境）
3. 在 Configs 面板按 `n` 添加配置项（key-value，value 支持任意 JSON 类型）
4. 在 API Keys 面板按 `n` 生成 API Key
5. 在 Server 面板按 `s` 启动 REST API 服务
6. 使用 API Key 通过 REST API 读取配置

## REST API

认证方式：`X-API-Key` 请求头，API Key 绑定到具体项目。

### 获取合并后的全部配置

合并逻辑：项目配置 + 共享组配置，项目配置优先覆盖。

```bash
curl -H "X-API-Key: YOUR_API_KEY" \
  http://localhost:3000/api/v1/projects/my-app/envs/prod/configs
```

响应：
```json
{"project": "my-app", "environment": "prod", "configs": {"db_host": "localhost", "log_level": "info"}}
```

### 获取单个配置项

```bash
curl -H "X-API-Key: YOUR_API_KEY" \
  http://localhost:3000/api/v1/projects/my-app/envs/prod/configs/db_host
```

响应：
```json
{"key": "db_host", "value": "localhost"}
```

### 错误响应

- 缺少或无效 API Key → 401
- API Key 与请求项目不匹配 → 403
- 项目/环境/配置项不存在 → 404

```json
{"error": "project not found: my-app"}
```

## 数据文件

数据持久化到运行目录下的 `data.json` 文件。支持通过 Import/Export 功能进行全量状态的 JSON 导入导出，可用于分布式同步。

## 测试

```bash
cargo test
```

共 124 个单元测试，覆盖所有模块。
