# AGENTS.md

本文件为 Codex (Codex.ai/code) 提供本仓库的开发指引。

## 重要约束

**禁止执行任何编译/构建命令**（包括但不限于 `cargo build`、`cargo check`、`cargo clippy`、`npx tauri build`、`npm run build`、`tsc` 等）。
完成代码修改后，必须提醒用户手动执行编译验证，例如：
> "代码已修改完毕，请运行 `cargo clippy --workspace -- -D warnings` 和 `npx tsc --noEmit` 验证编译。"

## 常用命令

```bash
# 桌面应用开发（Tauri + Vite HMR，必须用 tauri dev 而不是单独 npm run dev）
npx tauri dev

# Web 服务器开发
npm run build:web          # 构建前端（Web 模式）
cargo run -p session-web   # 启动 Axum 服务器

# 构建生产包（MSI/NSIS/DMG/DEB + 更新签名）
# 需要环境变量 TAURI_SIGNING_PRIVATE_KEY 和 TAURI_SIGNING_PRIVATE_KEY_PASSWORD
npx tauri build

# 构建 Web 服务器
npm run build:web && cargo build -p session-web --release

# 本地开发/构建（带签名，PowerShell 脚本，已 gitignore）
.\dev.ps1
.\build.ps1

# 代码检查
cargo clippy --workspace -- -D warnings   # Rust lint（全 workspace）
npx tsc --noEmit                           # TypeScript 类型检查

# 前端构建（tauri 会自动调用）
npm run build    # 版本校验 + 图标生成 + tsc -b + vite build

# 将 package.json 版本号同步到所有 Cargo.toml 和 tauri.conf.json
npm run sync-version
```

## 架构

Cargo Workspace 包含三个 crate：

- **`crates/session-core`** — 共享 Rust 核心库（models/provider/parser/search/stats/state），无 Tauri 依赖
- **`src-tauri`** — Tauri v2 桌面应用后端，依赖 session-core
- **`crates/session-web`** — Axum HTTP 服务器 + WebSocket，依赖 session-core，rust-embed 嵌入前端

前端通过编译时变量 `__IS_TAURI__`（Vite define）自动切换 API 层：
- Tauri 模式：`tauriApi.ts`（invoke IPC）
- Web 模式：`webApi.ts`（HTTP fetch + WebSocket）
- 统一入口：`api.ts`（动态 import）

### 三数据源模式

会话 API 接收 `source` 参数（`"claude" | "codex" | "grok"`），调度到对应 `provider/`：

- `provider/claude.rs` — 读取 `~/.claude/projects/{encoded-path}/`（sessions-index.json + *.jsonl）
- `provider/codex.rs` — 读取 `~/.codex/sessions/{year}/{month}/{day}/rollout-*.jsonl`
- `provider/grok.rs` — 读取 `$GROK_HOME/sessions/` 或 `~/.grok/sessions/` 下的 `summary.json` + `chat_history.jsonl`

三个 provider 解析为共享的 `models/` 类型（`ProjectEntry`、`SessionIndexEntry`、`DisplayMessage`）。Grok 仅接入本地会话浏览、搜索、导出、删除/回收站、恢复和文件监听；CLI 对话、Token 统计与 Provider 同步仍只支持原有数据源。

### 前端 → 后端通信

```
# Tauri 桌面模式
React 组件 → appStore.ts → api.ts → tauriApi.ts → invoke() → Tauri command → session-core → 文件系统

# Web 服务器模式
React 组件 → appStore.ts → api.ts → webApi.ts → fetch() → Axum route → session-core → 文件系统
```

### 消息分页

消息默认从**末尾**加载（`fromEnd: true`）。`page=0` = 最后一页，`page=1` = 倒数第二页。分页计算使用 `saturating_sub` 模式。JSONL 文件通过 `BufReader` 逐行解析。

### 状态管理

- `appStore.ts` — 主应用状态（source、projects、sessions、messages、search、stats）
- `updateStore.ts` — 独立的自动更新系统 store（`__IS_TAURI__` 守卫）
- 切换数据源（`setSource`）会清空所有下游状态（projects、sessions、messages）

### 后端状态

`AppState` 持有 LRU 缓存（容量 20），以文件路径为 key。通过 `parking_lot::Mutex` 保护。位于 `session-core/src/state.rs`。

### 更新系统

混合更新方式：安装版（MSI/NSIS/DMG/DEB）使用 `tauri-plugin-updater` 应用内下载安装；便携版（Windows ZIP）检测更新后打开 GitHub Release 页面。`get_install_type` 命令通过检查 NSIS 卸载器来区分安装类型。Web 模式下自动隐藏更新相关 UI。

### 主题系统

CSS 变量定义在 `index.css`（`:root` 浅色，`.dark` 深色）。Tailwind 通过 `tailwind.config.js` 的 theme 扩展引用这些变量。偏好存储在 localStorage。

## 开发约定

- Rust 命令返回 `Result<T, String>` — 错误转字符串传给前端
- Model 使用 `#[serde(rename_all = "camelCase")]` 做 JSON ↔ Rust 字段映射
- React 组件统一使用函数组件 + hooks
- URL 参数中的项目 ID 经过 `encodeURIComponent` 编码（路径含特殊字符）
- UI 文字使用中文（简体中文）
- 图标来自 `lucide-react`，样式使用 Tailwind CSS 工具类
- 字体：Inter（正文）+ JetBrains Mono（代码），内嵌 woff2
- Web 模式条件渲染使用 `__IS_TAURI__` 编译时变量

## 构建脚本

位于 `scripts/` 目录：

- **`generate-icons.mjs`** — 读取 `public/logo.png`，对比与 `src-tauri/icons/icon.png` 的修改时间，仅在 logo 变更时执行 `tauri icon` 重新生成。已挂载到 `dev` 和 `build` 脚本。
- **`sync-version.mjs`** — 版本号唯一来源：`package.json`。`sync` 模式写入 3 个 `Cargo.toml`（src-tauri、session-core、session-web）+ `tauri.conf.json`；`check` 模式（`build` 时使用）版本不一致则报错阻止构建。

## 测试环境

- **Rocky 9 开发机**：VMware 虚拟机，通过 `ssh root@192.168.124.133` 访问，用于 Linux 平台测试

### Rocky 9 常见问题

**端口被占用（Address in use）**

`session-web` 默认绑定 **0.0.0.0**（所有网卡，可通过 `--host` 或 `ASV_HOST` 修改），默认监听 **3000** 端口（可通过 `--port` 或 `ASV_PORT` 环境变量修改）。
若启动报 `Failed to bind address: Os { code: 98, kind: AddrInUse }`，执行：

```bash
# 查看占用进程
ss -tlnp | grep :3000

# 杀掉残留的 session-web 进程
pkill -f session-web

# 或换端口启动
ASV_PORT=8080 ./session-web
./session-web --port 8080
```

## 发布

标签触发：`git tag v1.x.0 && git push origin v1.x.0`。CI 构建：
- 4 个平台桌面应用（Windows、macOS ARM、macOS Intel、Linux）+ `.sig` 签名 + `latest.json`
- Web 服务器 Linux x86_64 二进制（tar.gz）
- Docker 镜像推送到 GHCR

版本工作流：修改 `package.json` 中的 version → 执行 `npm run sync-version` → 提交并打标签。构建时会自动校验版本一致性，不一致则阻止构建。
