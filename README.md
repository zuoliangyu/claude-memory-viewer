# Claude Memory Viewer

<p align="center">
  <img src="src-tauri/icons/icon.png" width="128" height="128" alt="Claude Memory Viewer">
</p>

<p align="center">
  <strong>Claude Code 本地会话记忆的可视化浏览器</strong>
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

[Claude Code](https://docs.anthropic.com/en/docs/claude-code) 是 Anthropic 官方的 CLI 编程助手。它将所有会话数据以 JSONL 格式存储在本地 `~/.claude/projects/` 目录下，但没有内置的可视化浏览方式。

**Claude Memory Viewer** 是一个轻量级桌面应用，让你可以直观地浏览、搜索、统计所有 Claude Code 的会话记忆，并支持一键恢复（Resume）到 CLI 中继续对话。

## Screenshots

![图片](./img/1.png)
![图片](./img/2.png)
![图片](./img/3.png)

## Features

### Project Browser

- 自动扫描 `~/.claude/projects/` 目录，列出所有使用过 Claude Code 的项目
- 显示每个项目的会话数量、最后活跃时间
- 按最近活跃时间排序，快速找到目标项目

### Session List

- 读取 Claude Code 的 `sessions-index.json` 索引文件，毫秒级加载
- 展示每个会话的摘要（summary）、首条 Prompt、消息数量、Git 分支、创建/修改时间
- 支持按时间排序和过滤

### Message Detail

- 完整渲染会话中的所有消息：
  - **用户消息** — 原始输入
  - **AI 回复** — Markdown 排版 + 语法高亮
  - **思考过程**（Thinking） — 可折叠展开
  - **工具调用**（Tool Use） — 调用名称、参数、返回结果
  - **代码块** — 多语言语法高亮
- 分页加载，大会话（上千条消息）也不会卡顿
- 虚拟滚动优化渲染性能

### Resume Session

- 选中任意会话，一键在系统终端中执行 `claude --resume {sessionId}`
- 终端完全独立于本应用 — 关闭 Viewer 后终端继续运行
- 跨平台支持：
  - **Windows** — 通过 `cmd /c start /d` 启动独立终端进程
  - **macOS** — 通过 AppleScript 调用 Terminal.app（由 Terminal.app 持有进程）
  - **Linux** — 自动检测 gnome-terminal / konsole / xfce4-terminal / xterm，通过 `setsid` 脱离父进程

### Global Search

- 跨所有项目、所有会话全文搜索
- 基于 Rayon 并行扫描，搜索速度快
- 关键词高亮，点击结果直接跳转到对应消息

### Token Statistics

- 读取 Claude Code 的 `stats-cache.json` 统计缓存
- 展示：
  - 每日活动图表（消息数、会话数）
  - Token 用量趋势（input / output / cache）
  - 按模型分组的 Token 消耗
  - 活跃时段分布
- 支持按项目、按会话粒度查看

### Real-time Update

- 使用 `notify` crate 监听 `~/.claude/projects/` 文件系统变化
- 新会话创建、会话更新时自动刷新界面，无需手动操作

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | [Tauri v2](https://v2.tauri.app/) (Rust + WebView) |
| Frontend | React 19 + TypeScript + Vite 6 |
| Styling | Tailwind CSS 3 + @tailwindcss/typography |
| State | Zustand 5 |
| Virtual Scroll | @tanstack/react-virtual 3 |
| Markdown | react-markdown 9 + remark-gfm + react-syntax-highlighter |
| Charts | Recharts 2 |
| Icons | Lucide React |
| Date | date-fns 4 |
| File Watch | notify 7 (Rust) |
| Parallel Search | Rayon 1.10 (Rust) |
| Cache | LRU 0.12 (Rust) |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri WebView                         │
│  ┌───────────┐  ┌──────────────┐  ┌──────────────────┐ │
│  │  Sidebar   │  │  Session List │  │  Message Thread  │ │
│  │ (Projects) │  │  (Index View) │  │  (JSONL Parsed)  │ │
│  └─────┬─────┘  └──────┬───────┘  └────────┬─────────┘ │
│        │               │                    │            │
│  ┌─────┴───────────────┴────────────────────┴─────────┐ │
│  │              Zustand Store + tauriApi.ts            │ │
│  └────────────────────────┬───────────────────────────┘ │
└───────────────────────────┼─────────────────────────────┘
                            │  Tauri IPC (invoke)
┌───────────────────────────┼─────────────────────────────┐
│  Rust Backend             │                              │
│  ┌────────────────────────┴───────────────────────────┐ │
│  │                  Tauri Commands                     │ │
│  │  get_projects  get_sessions  get_messages           │ │
│  │  global_search  get_global_stats  resume_session    │ │
│  └──────┬──────────────┬──────────────────┬───────────┘ │
│         │              │                  │              │
│  ┌──────┴─────┐ ┌──────┴──────┐ ┌────────┴──────────┐  │
│  │   Parser   │ │   Watcher   │ │   Terminal Spawn   │  │
│  │ (JSONL +   │ │ (notify +   │ │ (cmd/osascript/    │  │
│  │  BufReader)│ │  events)    │ │  gnome-terminal)   │  │
│  └──────┬─────┘ └─────────────┘ └────────────────────┘  │
│         │                                                │
│  ┌──────┴───────────────────────────────────────────┐   │
│  │           ~/.claude/projects/                     │   │
│  │  sessions-index.json  *.jsonl  stats-cache.json   │   │
│  └───────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

## Data Source

本应用**只读取本地文件**，不联网、不上传任何数据。

Claude Code 的本地存储结构：

```
~/.claude/
├── projects/
│   └── {encoded-project-path}/
│       ├── sessions-index.json          # 会话索引（摘要、消息数、时间）
│       ├── {sessionId}.jsonl            # 会话完整历史
│       └── {sessionId}/
│           ├── subagents/               # 子代理会话
│           │   └── agent-{id}.jsonl
│           └── tool-results/            # 工具执行结果
│               └── {toolId}.txt
├── stats-cache.json                     # 全局使用统计
├── history.jsonl                        # 全局历史索引
└── settings.json                        # 用户设置
```

### Key Data Formats

**sessions-index.json** — 会话索引（列表页数据源）：

```json
{
  "version": 1,
  "entries": [
    {
      "sessionId": "4ec3dc60-...",
      "summary": "Feature Implementation",
      "firstPrompt": "Help me implement...",
      "messageCount": 38,
      "created": "2026-01-22T13:48:02.915Z",
      "modified": "2026-01-22T14:07:36.022Z",
      "gitBranch": "main"
    }
  ]
}
```

**{sessionId}.jsonl** — 会话历史（每行一条 JSON 记录）：

```json
{
  "type": "assistant",
  "sessionId": "4ec3dc60-...",
  "message": {
    "role": "assistant",
    "content": [
      { "type": "thinking", "thinking": "..." },
      { "type": "text", "text": "..." },
      { "type": "tool_use", "name": "Read", "input": { "file_path": "..." } }
    ],
    "model": "claude-opus-4-6",
    "usage": { "input_tokens": 1234, "output_tokens": 567 }
  },
  "timestamp": "2026-01-22T13:48:05.123Z"
}
```

## Prerequisites

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/tools/install) >= 1.75
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 已使用过（`~/.claude/projects/` 目录存在）

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
claude-memory-viewer/
├── src/                              # Frontend (React + TypeScript)
│   ├── App.tsx                       # 路由配置
│   ├── components/
│   │   ├── layout/                   # AppLayout, Sidebar
│   │   ├── project/                  # ProjectsPage - 项目列表
│   │   ├── session/                  # SessionsPage - 会话列表
│   │   ├── message/                  # MessagesPage, MessageThread
│   │   │                             # AssistantMessage, UserMessage
│   │   ├── search/                   # SearchPage - 全局搜索
│   │   └── stats/                    # StatsPage - 统计面板
│   ├── stores/appStore.ts            # Zustand 全局状态
│   ├── services/tauriApi.ts          # Tauri invoke 封装
│   └── types/index.ts                # TypeScript 类型定义
│
├── src-tauri/                        # Backend (Rust)
│   ├── Cargo.toml
│   ├── tauri.conf.json               # Tauri 配置
│   ├── capabilities/default.json     # 权限声明
│   ├── icons/                        # 应用图标（多尺寸）
│   └── src/
│       ├── main.rs                   # 入口
│       ├── lib.rs                    # Tauri Builder + 命令注册
│       ├── state.rs                  # AppState（LRU 缓存）
│       ├── commands/
│       │   ├── projects.rs           # get_projects - 扫描项目
│       │   ├── sessions.rs           # get_sessions - 读取会话索引
│       │   ├── messages.rs           # get_messages - 分页加载消息
│       │   ├── search.rs             # global_search - 并行搜索
│       │   ├── stats.rs              # get_global_stats - 统计数据
│       │   └── terminal.rs           # resume_session - 跨平台终端启动
│       ├── models/                   # 数据结构定义
│       │   ├── project.rs            # Project
│       │   ├── session.rs            # SessionsIndex, SessionIndexEntry
│       │   ├── message.rs            # RawRecord, ContentValue, DisplayMessage
│       │   └── stats.rs              # StatsCache, TokenUsageSummary
│       ├── parser/
│       │   ├── jsonl.rs              # 流式 JSONL 解析（BufReader + 行级预过滤）
│       │   └── path_encoder.rs       # Claude home 定位 + 路径处理
│       └── watcher/
│           └── fs_watcher.rs         # notify crate 文件系统监听
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

后端通过 Tauri IPC 暴露以下命令供前端调用：

| Command | Parameters | Returns | Description |
|---------|-----------|---------|-------------|
| `get_projects` | — | `Project[]` | 扫描所有项目 |
| `get_sessions` | `encodedName` | `SessionIndexEntry[]` | 获取项目会话列表 |
| `get_session_detail` | `encodedName, sessionId` | `SessionDetail` | 获取会话详情 |
| `get_messages` | `encodedName, sessionId, page, pageSize` | `PaginatedMessages` | 分页加载消息 |
| `global_search` | `query, maxResults` | `SearchResult[]` | 全局搜索 |
| `get_global_stats` | — | `StatsCache` | 全局统计 |
| `get_project_token_stats` | `encodedName` | `TokenUsageSummary` | 项目 Token 统计 |
| `resume_session` | `sessionId, projectPath` | `()` | 终端中恢复会话 |

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
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions 会自动：

1. 在 Windows、macOS (Intel + Apple Silicon)、Linux 上并行构建
2. 生成各平台安装包
3. 创建 GitHub Release Draft 并上传所有安装包

## Roadmap

- [x] 项目骨架搭建（Tauri v2 + React + Vite + Tailwind）
- [ ] 项目列表浏览
- [ ] 会话列表（基于 sessions-index.json）
- [ ] 消息详情渲染（Markdown / 代码高亮 / 工具调用 / 思考过程）
- [ ] Resume 会话（跨平台终端启动）
- [ ] 文件系统监听 + 实时更新
- [ ] 全局搜索
- [ ] Token 统计面板
- [ ] 暗色 / 亮色主题切换
- [ ] 自定义标题栏
- [ ] GitHub Actions 多平台自动构建

## Contributing

欢迎提交 Issue 和 Pull Request。

```bash
# 开发前请确保通过以下检查
cd src-tauri && cargo clippy -- -D warnings   # Rust lint
npx tsc --noEmit                               # TypeScript 类型检查
```

## License

[MIT](LICENSE)
