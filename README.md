# AI Session Viewer

<p align="center">
  <img src="src-tauri/icons/icon.png" width="128" height="128" alt="AI Session Viewer">
</p>

<p align="center">
  <strong>Claude Code & Codex CLI 本地会话记忆的统一可视化浏览器</strong>
</p>

<p align="center">
  <a href="https://github.com/zuoliangyu/claude-memory-viewer/releases">
    <img src="https://img.shields.io/github/v/release/zuoliangyu/claude-memory-viewer?style=flat-square" alt="Release">
  </a>
  <a href="https://github.com/zuoliangyu/claude-memory-viewer/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/zuoliangyu/claude-memory-viewer/build.yml?style=flat-square&label=CI" alt="CI">
  </a>
  <a href="https://github.com/zuoliangyu/claude-memory-viewer/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/zuoliangyu/claude-memory-viewer?style=flat-square" alt="License">
  </a>
</p>

---

**AI Session Viewer** 是一个轻量级桌面应用，让你可以在一个统一界面中浏览、搜索、统计来自 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 和 [OpenAI Codex CLI](https://github.com/openai/codex) 的所有本地会话记忆，并支持一键恢复（Resume）到对应 CLI 中继续对话。

本应用**只读取本地文件**，不联网、不上传任何数据。

## Screenshots

![图片](./img/1.png)
![图片](./img/2.png)
![图片](./img/3.png)
![图片](./img/4.png)
![图片](./img/5.png)
![图片](./img/6.png)

## Features

### Dual Data Source

通过侧边栏顶部的 Tab 一键切换 Claude / Codex 数据源：

| 数据源 | CLI 工具 | 本地存储路径 | 特色 |
|--------|---------|-------------|------|
| **Claude** (橙色主题) | [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | `~/.claude/projects/` | Thinking 块、Tool Use、sessions-index 索引 |
| **Codex** (绿色主题) | [Codex CLI](https://github.com/openai/codex) | `~/.codex/sessions/` | Reasoning 块、Function Call、按日期归档 |

切换时自动清理状态并重新加载，互不干扰。

### Project Browser

- 自动扫描对应数据源目录，列出所有项目
- Claude：按 `~/.claude/projects/{encoded-path}` 聚合
- Codex：按会话元数据中的 `cwd` 工作目录聚合
- 显示每个项目的会话数量、最后活跃时间
- 按最近活跃时间排序

### Session List

- Claude：读取 `sessions-index.json` 索引文件并与磁盘 `.jsonl` 文件合并，确保 Ctrl+C 退出的会话不会丢失
- Codex：扫描 `~/.codex/sessions/` 目录下所有 `rollout-*.jsonl` 文件，提取元数据
- 展示每个会话的首条 Prompt、消息数量、Git 分支、创建/修改时间
- 支持删除会话（带确认弹窗）

### Message Detail

完整渲染会话中的所有消息，支持两种 AI 的不同内容块格式：

| 内容块类型 | Claude | Codex | 说明 |
|-----------|--------|-------|------|
| Text | ✅ | ✅ | Markdown 渲染 + 语法高亮 |
| Thinking | ✅ | — | Claude 思考过程，可折叠 |
| Reasoning | — | ✅ | Codex 推理过程，可折叠 |
| Tool Use | ✅ | — | 工具调用名称、参数、返回结果 |
| Tool Result | ✅ | — | 工具返回结果 |
| Function Call | — | ✅ | Codex 函数调用 |
| Function Call Output | — | ✅ | 函数调用返回结果 |

- 分页加载，大会话（上千条消息）也不会卡顿
- 默认从最新消息加载，进入会话直接看到最近对话
- 向上滚动自动加载更早的消息，滚动位置自动保持
- 浮动"跳转到顶部/底部"双向按钮
- 时间戳 / 模型标签可切换显示，偏好持久化

### Resume Session

选中任意会话，一键在系统终端中恢复：

- **Claude** → 执行 `claude --resume {sessionId}`
- **Codex** → 执行 `codex resume {sessionId}`

终端完全独立于本应用——关闭 Viewer 后终端继续运行。跨平台支持：

| 平台 | 实现方式 |
|------|---------|
| Windows | `cmd /c start /d` 启动独立终端进程 |
| macOS | AppleScript 调用 Terminal.app |
| Linux | 自动检测 gnome-terminal / konsole / xfce4-terminal / xterm，`setsid` 脱离父进程 |

### Global Search

- 在当前数据源下跨所有项目、所有会话全文搜索
- 基于 Rayon 并行扫描 JSONL 文件
- UTF-8 安全的字符级切片，中文/emoji 不会崩溃
- 关键词高亮，点击结果直接跳转到对应消息

### Token Statistics

- Claude：读取 `stats-cache.json` 统计缓存
- Codex：从每个会话文件提取 `usage` 字段聚合
- 展示：会话总数、消息总数、Input/Output Token 用量
- 每日 Token 用量柱状图
- Token 趋势面积图
- 按模型分组的 Token 消耗

### Auto Update

应用内自动检测更新，安装版和便携版分别处理：

| 安装方式 | 更新行为 |
|---------|---------|
| **安装版** (MSI/NSIS/DMG/DEB) | 应用内一键下载 + 自动安装 + 重启 |
| **便携版** (Windows Portable ZIP) | 检测到新版后引导跳转 GitHub Release 下载 |

- 启动后自动检查，Sidebar 底部显示版本号 + 手动检查按钮
- 有更新时版本号旁显示蓝色脉冲圆点，点击展开内嵌更新面板
- 支持忽略特定版本，不再重复提示
- 基于 `tauri-plugin-updater` + Ed25519 签名验证，确保更新包完整性

### Real-time Update

- 使用 `notify` crate 同时监听两个目录的文件系统变化
- 新会话创建、会话更新时自动刷新界面

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | [Tauri v2](https://v2.tauri.app/) (Rust + WebView) |
| Frontend | React 19 + TypeScript + Vite 6 |
| Styling | Tailwind CSS 3 + @tailwindcss/typography |
| State | Zustand 5 |
| Markdown | react-markdown 9 + remark-gfm + react-syntax-highlighter |
| Charts | Recharts 2 |
| Icons | Lucide React |
| Date | date-fns 4 |
| File Watch | notify 7 (Rust) |
| Parallel Search | Rayon 1.10 (Rust) |
| Cache | LRU 0.12 (Rust) |
| Auto Update | tauri-plugin-updater 2 + tauri-plugin-process 2 (Rust) |

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                      Tauri WebView                            │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐ │
│  │   Sidebar     │  │  Session List │  │   Message Thread   │ │
│  │ [Claude|Codex]│  │  (Index View) │  │  (JSONL Parsed)    │ │
│  │  + Projects   │  │              │  │                    │ │
│  └──────┬───────┘  └──────┬───────┘  └────────┬───────────┘ │
│         │                 │                    │              │
│  ┌──────┴─────────────────┴────────────────────┴───────────┐ │
│  │          Zustand Store (source: claude|codex)            │ │
│  │                    tauriApi.ts                            │ │
│  └──────────────────────────┬──────────────────────────────┘ │
└─────────────────────────────┼────────────────────────────────┘
                              │  Tauri IPC (invoke + source)
┌─────────────────────────────┼────────────────────────────────┐
│  Rust Backend               │                                 │
│  ┌──────────────────────────┴──────────────────────────────┐ │
│  │                   Tauri Commands                         │ │
│  │  get_projects(source)   get_sessions(source, id)         │ │
│  │  get_messages(source, path)   global_search(source, q)   │ │
│  │  get_token_stats(source)   resume_session(source, id)    │ │
│  └────┬───────────────────────────────────────────┬────────┘ │
│       │                                           │          │
│  ┌────┴──────────────┐                  ┌─────────┴────────┐ │
│  │  provider/claude   │                  │  provider/codex   │ │
│  │  (JSONL + Index)   │                  │  (JSONL + Meta)   │ │
│  └────┬──────────────┘                  └─────────┬────────┘ │
│       │                                           │          │
│  ┌────┴───────────────────────────────────────────┴────────┐ │
│  │  ~/.claude/projects/          ~/.codex/sessions/         │ │
│  │  sessions-index.json *.jsonl  rollout-*.jsonl            │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                               │
│  ┌────────────────┐  ┌────────────────────────────────────┐  │
│  │    Watcher      │  │        Terminal Spawn               │  │
│  │ (notify: dual   │  │ (cmd / osascript / gnome-terminal) │  │
│  │  dir watch)     │  │ (claude --resume / codex resume)   │  │
│  └────────────────┘  └────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

## Data Source

### Claude Code

```
~/.claude/
├── projects/
│   └── {encoded-project-path}/
│       ├── sessions-index.json          # 会话索引
│       ├── {sessionId}.jsonl            # 会话完整历史
│       └── {sessionId}/
│           ├── subagents/               # 子代理会话
│           └── tool-results/            # 工具执行结果
├── stats-cache.json                     # 全局使用统计
└── settings.json                        # 用户设置
```

### Codex CLI

```
~/.codex/
└── sessions/
    └── {year}/{month}/{day}/
        └── rollout-{timestamp}-{id}.jsonl   # 每个文件 = 一个会话
            # 第一行: session_meta (cwd, model, cli_version, ...)
            # 后续行: response_item (message/function_call/function_call_output)
```

## Prerequisites

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/tools/install) >= 1.75
- 至少使用过以下一种 CLI：
  - [Claude Code](https://docs.anthropic.com/en/docs/claude-code)（`~/.claude/projects/` 目录存在）
  - [Codex CLI](https://github.com/openai/codex)（`~/.codex/sessions/` 目录存在）

### Platform-specific

**Windows:**
- [Microsoft Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (Windows 10/11 通常已内置)

**macOS:**
- Xcode Command Line Tools: `xcode-select --install`

**Linux (Ubuntu/Debian):**
```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

## Development

```bash
# 克隆仓库
git clone https://github.com/zuoliangyu/claude-memory-viewer.git
cd claude-memory-viewer

# 安装前端依赖
npm install

# 启动开发服务器（同时启动 Vite 前端 + Rust 后端）
npx tauri dev
```

> **注意**: 不能只运行 `npm run dev`，那只会启动 Vite 前端。必须用 `npx tauri dev` 才能同时编译 Rust 后端并启动完整应用。

### Project Structure

```
AI-Session-Viewer/
├── src/                              # Frontend (React + TypeScript)
│   ├── App.tsx                       # 路由配置
│   ├── components/
│   │   ├── layout/                   # AppLayout, Sidebar（含 Claude/Codex Tab）
│   │   ├── project/                  # ProjectsPage - 项目列表
│   │   ├── session/                  # SessionsPage - 会话列表
│   │   ├── message/                  # MessagesPage, MessageThread
│   │   │                             # AssistantMessage, UserMessage
│   │   │                             # ToolOutputMessage
│   │   ├── search/                   # SearchPage - 全局搜索
│   │   └── stats/                    # StatsPage - 统计面板
│   ├── stores/appStore.ts            # Zustand 全局状态（含 source 切换）
│   ├── services/tauriApi.ts          # Tauri invoke 封装（所有 API 含 source 参数）
│   └── types/index.ts                # TypeScript 类型定义
│
├── src-tauri/                        # Backend (Rust)
│   ├── Cargo.toml
│   ├── tauri.conf.json               # Tauri 配置
│   ├── capabilities/default.json     # 权限声明
│   ├── icons/                        # 应用图标
│   └── src/
│       ├── main.rs                   # 入口
│       ├── lib.rs                    # Tauri Builder + 命令注册
│       ├── state.rs                  # AppState（LRU 缓存）
│       ├── commands/
│       │   ├── projects.rs           # get_projects(source) - 扫描项目
│       │   ├── sessions.rs           # get_sessions(source) - 读取会话索引
│       │   ├── messages.rs           # get_messages(source) - 分页加载消息
│       │   ├── search.rs             # global_search(source) - 并行搜索
│       │   ├── stats.rs              # get_token_stats(source) - Token 统计
│       │   ├── terminal.rs           # resume_session(source) - 跨平台终端启动
│       │   └── updater.rs            # get_install_type - 检测安装类型
│       ├── provider/                  # 双数据源提供层
│       │   ├── claude.rs             # Claude Code 数据解析
│       │   └── codex.rs              # Codex CLI 数据解析
│       ├── models/                   # 统一数据结构
│       │   ├── project.rs            # ProjectEntry（含 source 字段）
│       │   ├── session.rs            # SessionIndexEntry（统一 Claude/Codex）
│       │   ├── message.rs            # DisplayMessage + 7种内容块枚举
│       │   └── stats.rs              # TokenUsageSummary
│       ├── parser/
│       │   └── path_encoder.rs       # Claude home 定位 + 路径处理
│       └── watcher/
│           └── fs_watcher.rs         # 双目录文件系统监听
│
├── .github/workflows/
│   ├── build.yml                     # CI: cargo check + clippy + tsc
│   └── release.yml                   # CD: 多平台构建 + GitHub Release
│
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── tsconfig.json
```

### Tauri Commands API

所有命令通过 `source` 参数区分数据源，由命令层调度到对应的 provider：

| Command | Parameters | Returns | Description |
|---------|-----------|---------|-------------|
| `get_projects` | `source` | `ProjectEntry[]` | 扫描指定数据源的所有项目 |
| `get_sessions` | `source, projectId` | `SessionIndexEntry[]` | 获取项目会话列表 |
| `get_messages` | `source, filePath, page, pageSize` | `PaginatedMessages` | 分页加载消息 |
| `global_search` | `source, query, maxResults` | `SearchResult[]` | 全局搜索 |
| `get_token_stats` | `source` | `TokenUsageSummary` | Token 统计 |
| `get_project_token_stats` | `source, projectId` | `TokenUsageSummary` | 项目 Token 统计 |
| `resume_session` | `source, sessionId, projectPath, filePath?` | `()` | 终端中恢复会话 |
| `delete_session` | `filePath` | `()` | 删除会话文件 |
| `get_install_type` | — | `String` | 检测安装版/便携版 |

## Build

```bash
# 构建生产版本安装包
npx tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`：

| Platform | Output |
|----------|--------|
| Windows | `.msi` + `.exe` (NSIS installer) |
| macOS | `.dmg` + `.app` |
| Linux | `.deb` + `.AppImage` |

## Release

项目使用 GitHub Actions 自动化构建和发布。创建一个 `v*` 格式的 tag 即可触发多平台构建：

```bash
git tag v1.0.0
git push origin v1.0.0
```

GitHub Actions 会自动：

1. 在 Windows、macOS (Intel + Apple Silicon)、Linux 上并行构建
2. 生成各平台安装包 + `.sig` 签名文件
3. 生成 `latest.json` 更新清单（供旧版客户端检测新版本）
4. 创建 GitHub Release 并上传所有产物

## Roadmap

- [x] 项目骨架搭建（Tauri v2 + React + Vite + Tailwind）
- [x] 项目列表浏览
- [x] 会话列表（基于 sessions-index.json）
- [x] 消息详情渲染（Markdown / 代码高亮 / 工具调用 / 思考过程）
- [x] Resume 会话（跨平台终端启动）
- [x] 文件系统监听 + 实时更新
- [x] 全局搜索
- [x] Token 统计面板
- [x] GitHub Actions 多平台自动构建
- [x] 会话删除
- [x] **双数据源支持（Claude Code + Codex CLI）**
- [x] 修复 Ctrl+C 退出会话在列表中丢失的问题
- [x] 暗色 / 亮色主题切换（Cerulean Flow 青绿色调）
- [x] 反向加载消息（默认显示最新对话）
- [x] 内嵌 Inter + JetBrains Mono 字体
- [x] 消息卡片样式 + 用户消息 Markdown 渲染
- [x] 聊天气泡式消息布局
- [x] 消息显示模型名称 + 时间戳/模型切换按钮
- [x] 应用内自动更新（安装版自动安装 / 便携版引导下载）
- [ ] 自定义标题栏
- [ ] 更多 AI CLI 数据源支持（Gemini CLI 等）

## Contributing

欢迎提交 Issue 和 Pull Request。

```bash
# 开发前请确保通过以下检查
cd src-tauri && cargo clippy -- -D warnings   # Rust lint
npx tsc --noEmit                               # TypeScript 类型检查
```

## License

[MIT](LICENSE)
