# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [1.6.0] - 2026-02-26

### Added

#### CLI 对话模式
- 侧边栏新增「CLI 对话」入口，可直接在应用内与 Claude Code / Codex CLI 进行对话
- 自动检测本地已安装的 CLI 工具（Claude Code、Codex CLI）
- 支持选择工作目录、切换数据源（Claude/Codex）
- 流式输出：CLI 的 stdout 实时转发到前端，支持 Markdown 渲染
- 支持 `--resume` 继续已有会话
- 支持 `--dangerously-skip-permissions` 跳过权限确认
- 支持通过输入框 `/model` 命令自由切换模型（如 `/model claude-opus-4-6`）
- 消息详情页新增「继续对话」按钮，可在 CLI 对话模式中继续该会话

#### 快速问答模式
- 侧边栏新增「快速问答」入口，直接调用 Anthropic/OpenAI API 进行纯文本对话
- 无需选择工作目录，无 CLI 依赖
- 支持 Claude（Anthropic API）和 Codex（OpenAI API）双数据源
- SSE 流式输出，Markdown 实时渲染
- 独立对话历史，与 CLI 对话模式互不干扰

#### CLI 配置自动检测
- 新增 `cli_config` 模块，自动读取本地 CLI 配置文件获取 API Key 和 Base URL
- Claude：从 `~/.claude/settings.json` 读取 `ANTHROPIC_AUTH_TOKEN` / `ANTHROPIC_API_KEY` 和代理配置
- Codex：从 `~/.codex/auth.json` 读取 `OPENAI_API_KEY`，从 `~/.codex/config.toml` 读取模型和 provider 配置
- 模型列表获取无需手动输入 API Key，自动使用 CLI 配置
- 设置面板显示检测到的 CLI 配置状态（遮罩 Key、Base URL、默认模型）

#### 模型选择器增强
- 内置 Claude（Sonnet 4.6 / Opus 4.6 / Haiku 4.5）和 Codex（codex-mini / o4-mini / o3 / gpt-4.1）常用模型列表
- 支持 API 动态获取完整模型列表（有 API Key 时自动拉取）
- 搜索无结果时，按回车可直接使用搜索词作为自定义模型 ID

### Fixed

#### CLI 进程环境变量隔离
- **根因**：从 VS Code 终端启动 Tauri 开发服务器时，`CLAUDECODE` 等环境变量会被继承到 spawn 的 CLI 子进程，导致 CLI 运行异常（400 错误）
- **修复**：采用环境变量白名单机制（`env_clear()` + 仅传递 PATH、HOME 等必要系统变量），参考 opcode 项目的隔离方案

#### CLI 模型名 `-latest` 后缀不兼容
- Claude CLI 不接受 `-latest` 后缀的模型名（如 `claude-sonnet-4-6-latest`），传给 CLI 前自动剥离该后缀

### New Files

| 文件 | 说明 |
|------|------|
| `crates/session-core/src/cli.rs` | CLI 安装检测与路径查找 |
| `crates/session-core/src/cli_config.rs` | CLI 配置文件自动读取 |
| `crates/session-core/src/model_list.rs` | 模型列表获取（内置 + API） |
| `crates/session-core/src/quick_chat.rs` | 直接 API 流式对话 |
| `crates/session-web/src/chat_ws.rs` | Web 端 WebSocket 聊天路由 |
| `src-tauri/src/commands/chat.rs` | Tauri 聊天相关命令 |
| `src/components/chat/*.tsx` | CLI 对话页面组件（7 个） |
| `src/components/quick-chat/QuickChatPage.tsx` | 快速问答页面 |
| `src/stores/chatStore.ts` | CLI 对话状态管理 |
| `src/stores/quickChatStore.ts` | 快速问答状态管理 |
| `src/hooks/useChatStream.ts` | 聊天流事件监听 Hook |
| `src/types/chat.ts` | 聊天相关类型定义 |

---

## [1.5.0] - 2026-02-25

### Added

#### 全局搜索 — 会话分组模式
- 搜索页新增"消息 / 会话"分段切换按钮（搜索框下方）
- **会话模式**：搜索结果按会话（`filePath`）分组展示，每张卡片显示项目名、匹配数、最新时间、别名/首条 Prompt、标签 pill、前 3 条匹配文本摘要（带高亮），超出部分显示"还有 N 条匹配..."
- 点击会话卡片直接跳转到完整会话页面
- **消息模式**：保持原有逐条消息的平铺列表行为不变

#### 设置弹窗
- 侧边栏底部"关于作者"文字按钮替换为齿轮图标按钮
- 点击打开"设置"模态框，内含两个 Tab：
  - **使用说明**：分模块介绍侧边栏、项目列表、会话列表、消息详情、全局搜索、主题切换的操作方式
  - **关于作者**：保留原有作者信息（邮箱、QQ 群、哔哩哔哩、GitHub）

---

## [1.4.0] - 2026-02-25

### Added

#### 会话标签与别名系统
- 新增 `metadata.rs` 模块，每个项目的标签和别名持久化存储在 `.session-viewer-meta.json` 文件中
- 会话列表页（SessionsPage）支持为每个会话设置自定义别名和多个标签
- 新增 `SessionMetaEditor` 组件：弹窗编辑器，支持别名输入、标签管理（添加/删除）、已有标签自动补全
- 会话卡片显示标签 pill 和别名（别名替代首条 Prompt 作为标题，原 Prompt 显示为副标题）
- 消息详情页（MessagesPage）标题优先显示别名
- 文件监听器忽略 `.session-viewer-meta.json` 变更，避免编辑标签触发无限刷新

#### 跨项目标签筛选
- 新增 `get_all_cross_project_tags(source)` 后端接口，遍历所有项目收集去重标签
- 项目列表页（ProjectsPage）标题下方新增全局标签筛选栏，按标签过滤项目（仅显示拥有匹配标签的项目）
- 项目卡片显示该项目的标签 pill
- 搜索结果页（SearchPage）搜索框下方新增标签筛选栏，按标签过滤搜索结果
- 搜索结果 `SearchResult` 新增 `tags` 字段，搜索结果卡片显示标签 pill
- 会话列表页（SessionsPage）支持按标签筛选当前项目内的会话
- 切换数据源时自动清空所有标签筛选状态

#### REST API 扩展
- `PUT /api/sessions/meta` — 更新会话别名和标签
- `GET /api/tags` — 获取单个项目的所有标签
- `GET /api/cross-tags` — 获取跨项目的全局标签聚合

### Improved

#### 项目会话数统计更准确
- Claude 项目列表的会话数现在与会话列表一致：优先使用 `sessions-index.json` 索引统计有消息的会话，再加上磁盘上存在但不在索引中的非空文件
- 解决了之前"项目卡片显示 N 个会话"但进入后实际只有 M 个的不一致问题

#### 恢复会话按钮优化
- 所有恢复按钮（会话列表 + 消息详情页）统一支持右键复制恢复命令
- 按钮文字在"已复制"状态与默认状态间正确切换

---

## [1.3.1] - 2026-02-24

### Fixed

#### Docker 环境下项目列表"加载中"持续闪烁
- **根因**: Docker 挂载卷的 inotify 会频繁触发文件变化事件，每次事件通过 WebSocket 推送到前端后调用 `loadProjects()` 和 `selectProject()`，这两个函数都会设置 loading 状态并清空已有数据，导致"加载中"反复闪烁
- **修复**: 新增 `refreshInBackground()` 静默刷新方法，文件变化时仅更新数据而不触发 loading 状态；同时将前后端防抖时间从 300ms/500ms 统一提升至 1000ms，减少 Docker 环境下的事件风暴

---

## [1.3.0] - 2026-02-24

### Added

#### "关于作者"弹窗
- Sidebar 底部新增"关于作者"按钮（带边框文字按钮），点击弹出模态框
- 模态框展示作者信息：作者名称、邮箱、QQ 群号、哔哩哔哩主页、GitHub 仓库链接
- 每项带对应图标（lucide + 自定义 SVG），邮箱/哔哩哔哩/GitHub 均可点击跳转
- Tauri 桌面模式下使用 `@tauri-apps/plugin-shell` 打开外部链接，Web 模式下 fallback 到 `window.open`
- 点击背景遮罩或右上角关闭按钮均可关闭弹窗
- 浅色/暗色主题样式均适配

#### 前端版本号注入
- `vite.config.ts` 新增 `__APP_VERSION__` 编译时变量，从 `package.json` 读取版本号注入前端

### Fixed

#### 文件监听器删除会话后无限刷新
- 添加 debounce 防抖机制，防止删除会话后触发文件变更事件导致界面无限刷新

---

## [1.1.0] - 2026-02-24

### Added

#### Web 服务器变体（session-web）
- 新增 Axum HTTP 服务器，支持在无 GUI 的服务器环境通过浏览器远程访问会话数据
- 单文件可执行，前端通过 `rust-embed` 编译嵌入二进制中
- CLI 参数：`--host`、`--port`、`--token`（均支持环境变量 `ASV_HOST`/`ASV_PORT`/`ASV_TOKEN`）
- 可选 Bearer Token 认证，保护远程访问安全
- REST API：`/api/projects`、`/api/sessions`、`/api/messages`、`/api/search`、`/api/stats`
- WebSocket `/ws` 实时推送文件变更事件
- 新增 Docker 多阶段构建（`node:lts` → `rust:1` → `debian:bookworm-slim`）
- Docker 镜像自动推送到 GHCR（`ghcr.io/{repo}-web`）

#### Cargo Workspace 重构
- 提取共享 Rust 核心逻辑为 `crates/session-core`（models/provider/parser/search/stats/state）
- `src-tauri` 和 `crates/session-web` 共同依赖 `session-core`，消除代码重复
- 搜索逻辑（`search.rs`）和统计逻辑（`stats.rs`）从 Tauri commands 中提取为纯函数

#### 前端 API 层抽象
- 新增编译时变量 `__IS_TAURI__`（Vite define），自动区分桌面/Web 模式
- 新增 `src/services/webApi.ts`（HTTP fetch 封装）和 `src/services/api.ts`（统一入口）
- 前端组件 100% 复用，仅 API 调用层自动切换
- Web 模式下 Resume 按钮改为"复制恢复命令"到剪贴板
- Web 模式下自动隐藏更新检测相关 UI
- 新增 `src/hooks/useFileWatcher.ts`：Tauri 模式用事件监听，Web 模式用 WebSocket

#### CI/CD 扩展
- Release workflow 新增 `web-server` job：构建 Linux x86_64 Web 服务器二进制并上传到 Release
- Release workflow 新增 `docker` job：构建并推送 Docker 镜像到 GHCR
- Build workflow 新增 `check-web` job：独立检查 session-core + session-web 编译（无需 WebKit 系统依赖）

### Changed

- `sync-version.mjs` 现在同步版本号到 3 个 Cargo.toml（src-tauri、session-core、session-web）
- `build.yml` Rust cache workspaces 路径更新为 workspace 根目录
- `release.yml` portable zip 路径修正为 `target/release/`（workspace 模式下 target 在根目录）

---

## [1.0.1] - 2026-02-24

### Changed

#### 更新应用图标
- 替换 `public/logo.png` 源图，重新生成所有平台图标

#### 构建流程优化
- 新增 `scripts/generate-icons.mjs`：构建/开发时自动从 `public/logo.png` 生成全平台图标，仅在 logo 变更时执行
- 新增 `scripts/sync-version.mjs`：以 `package.json` 为版本号唯一来源，一键同步到 `Cargo.toml` + `tauri.conf.json`
- `npm run build` 自动校验三处版本号一致性，不一致则阻止构建
- 新增 `npm run sync-version` 命令

---

## [1.0.0] - 2026-02-24

### Added

#### 应用内更新系统（混合模式）
- **安装版**（MSI/NSIS/DMG/DEB）：集成 `tauri-plugin-updater`，支持应用内一键下载更新并自动重启
- **便携版**（Windows Portable ZIP）：检测到新版本后引导用户跳转 GitHub Release 页面下载
- 启动后 5 秒自动检查更新，每次会话仅检查一次
- Sidebar 底部新增版本号显示，有更新时显示蓝色脉冲圆点动画
- 点击版本号展开更新面板：显示版本变化、Release Notes、操作按钮
- 安装版显示"更新并重启"按钮 + 实时下载进度条
- 便携版显示"前往下载新版本"按钮，打开浏览器跳转 GitHub Release
- 支持"忽略此版本"功能，忽略后不再提示该版本（记忆到 localStorage）
- 新增 `get_install_type` Rust 命令：Windows 下检测 exe 同目录是否有 NSIS uninstaller 判断安装类型

#### CI/CD 自动签名
- Release workflow 注入 `TAURI_SIGNING_PRIVATE_KEY` 签名密钥
- 构建产物自动生成 `.sig` 签名文件和 `latest.json` 更新清单
- 旧版客户端可自动发现并验证新版本的完整性

### Changed

#### Sidebar Footer 布局调整
- 上排：项目数量统计 + 主题切换按钮
- 下排：版本号 + 手动检查更新按钮 + 可展开的更新面板
- 更新面板改为内嵌展开式（非弹窗），避免被 sidebar 滚动区域裁剪
- 新增手动检查更新按钮（刷新图标），用户可随时主动检查

---

## [0.8.0] - 2026-02-24

### Added

#### 消息显示模型名称
- 后端解析 JSONL 记录中的 `model` 字段并传递到前端
- AI 消息头部新增模型标签（如 `claude-sonnet-4-20250514`），一眼看清每条消息使用的模型
- Codex 消息暂无模型字段，保持兼容

#### 时间戳 / 模型标签切换按钮
- 消息页顶栏新增时钟和 CPU 图标按钮，可独立切换时间戳和模型标签的显示
- 偏好持久化到 localStorage，页面刷新后保持设置

#### 项目路径显示优化
- Claude 项目列表现在优先从 `sessions-index.json` 的 `originalPath` 读取真实项目路径
- 不再仅依赖目录名反向解码，解决含特殊字符的路径显示不准确的问题

### Changed

#### 聊天气泡式消息布局
- 用户消息改为右对齐气泡样式（`bg-primary/10` 圆角卡片），更贴近即时通讯体验
- AI 消息移除边框卡片，改为简洁的图标 + 内容布局
- 工具输出移除独立背景和圆形图标，改为紧凑的小图标 + 标签样式，视觉层级更清晰
- 消息线程最大宽度从 `max-w-4xl` 收窄到 `max-w-3xl`，消息间距从 `space-y-3` 增大到 `space-y-6`

#### 过滤空会话
- 会话列表现在自动过滤掉消息数为 0 的空会话
- 加载会话后同步更新项目卡片上的会话计数

### Fixed

#### Markdown 行内代码多余引号
- 修复 `@tailwindcss/typography` 为行内 `code` 标签自动添加反引号伪元素的问题
- 添加 CSS 规则移除 `::before` / `::after` 的 content，行内代码不再显示多余的引号

#### 会话卡片布局
- 会话列表卡片改用 `items-center` 垂直居中对齐，修复按钮和文字未对齐的问题

#### 思考/推理块图标溢出
- 为 Thinking 和 Reasoning 块的 Brain 图标添加 `shrink-0`，防止图标在窄屏下被压缩

---

## [0.7.0] - 2026-02-24

### Added

#### 反向加载消息 — 默认显示最新对话
- 后端 `get_messages` 新增 `from_end` 参数，支持从末尾分页
- 进入会话直接看到最新消息，自动滚到底部
- 向上滚动自动加载更早的消息，加载后保持滚动位置不跳
- 双向浮动按钮：跳转到顶部 / 跳转到底部，根据位置动态显示

#### 亮色 / 暗色主题切换
- 新增 Cerulean Flow 青绿色调主题（参考 E-FlowCode docs 配色）
- 支持三种模式：亮色 / 暗色 / 跟随系统
- 主题偏好持久化到 localStorage，页面加载无闪烁
- Sidebar 底部新增主题切换按钮组

#### 内嵌字体
- 内嵌 Inter（正文）和 JetBrains Mono（代码）woff2 字体
- 完全离线可用，不依赖 CDN

#### UI 样式优化
- 用户消息加 `bg-primary/5` 浅色背景卡片
- AI 消息加细边框卡片样式
- 工具输出左缩进 + 半透明背景，视觉上作为 AI 消息子级
- 用户消息支持 Markdown 渲染（行内代码等不再显示原始反引号）
- Markdown 段落间距优化

---

## [0.6.1] - 2026-02-10

### Fixed

#### Claude 每日 Token 用量图表空白
- **根因**: `stats-cache.json` 中的 `dailyModelTokens` 只有按模型汇总的 token 总量，没有 input/output 拆分。后端构建 `DailyTokenEntry` 时将 `input_tokens` 和 `output_tokens` 硬编码为 0，而前端柱状图仅渲染 input + output 的堆叠柱，导致图表看似空白
- **修复**: 解析 `stats-cache.json` 中的 `modelUsage` 字段获取全局 input/output 比例，按此比例将每日 token 总量分配为 input 和 output；同时修复摘要卡片中"输入 Token"始终显示为 0 的问题

#### Claude 恢复会话路径解析不准确
- **根因**: `resume_session` 仅使用前端传入的 `project_path`，该路径可能来自解码后的目录名而非真实项目路径，导致在终端中无法正确 `cd` 到项目目录
- **修复**: 新增 `file_path` 参数，从会话文件所在目录的 `sessions-index.json` 中读取 `originalPath` 作为优先项目路径；同时在恢复前将孤儿会话写入索引，确保 `claude --resume` 能发现该会话

---

## [0.6.0] - 2026-02-10

### Fixed

#### Ctrl+C 退出的会话在列表中丢失
- **根因**: Claude CLI 的 `sessions-index.json` 由 CLI 自身维护，当用户通过 Ctrl+C 强制退出时，CLI 可能来不及将当前会话写入索引。`get_sessions()` 优先信任索引，如果索引存在就只返回索引中的会话，忽略磁盘上存在但不在索引中的 `.jsonl` 文件
- **修复**: `get_sessions()` 现在执行合并逻辑——先读取索引条目，再扫描磁盘上所有 `.jsonl` 文件，对不在索引中的会话执行 fallback 扫描并合并到结果中

### Changed

#### 重构: 提取 `scan_single_session()` 函数
- 从 `scan_sessions_from_dir()` 的循环体中提取单文件扫描逻辑为独立的 `scan_single_session()` 函数
- `scan_sessions_from_dir()` 和合并逻辑共同复用此函数，消除代码重复

---

## [0.5.0] - 2026-02-07

### Highlights

**项目合并**: 将 `claude-memory-viewer` 和 `codex-session-viewer` 合并为统一应用 **AI Session Viewer**，在同一界面中同时支持 Claude Code 和 Codex CLI 两种 AI 编程助手的会话浏览。

### Added

#### Dual Data Source — Claude Code + Codex CLI
- 侧边栏顶部新增 Claude / Codex Tab 切换，一键切换数据源
- Claude Tab 使用橙色主题，Codex Tab 使用绿色主题
- 切换时自动清理所有状态（项目、会话、消息）并重新加载

#### Codex CLI Support
- 新增 `provider/codex.rs`，扫描 `~/.codex/sessions/{year}/{month}/{day}/rollout-*.jsonl`
- 按 `cwd` 工作目录聚合会话为项目
- 从 `session_meta` 首行提取元数据（cwd、model、cli_version）
- 支持 Codex 特有的消息格式：`reasoning` 推理块、`function_call` 函数调用、`function_call_output` 函数返回

#### Provider Architecture
- 新增 `provider/` 模块，将数据源解析从命令层解耦
- `provider/claude.rs` — 从原 `parser/jsonl.rs` 提取，处理 Claude Code 数据
- `provider/codex.rs` — 从 codex-session-viewer 移植，处理 Codex CLI 数据
- 所有 Tauri Commands 新增 `source` 参数，统一调度到对应 provider

#### Unified Models
- `DisplayContentBlock` 枚举扩展为 7 种变体：Text、Thinking、ToolUse、ToolResult、Reasoning、FunctionCall、FunctionCallOutput
- `ProjectEntry` 新增 `source`、`modelProvider` 字段
- `SessionIndexEntry` 新增 `source`、`filePath`、`cwd`、`modelProvider`、`cliVersion` 字段
- `TokenUsageSummary` 统一双数据源的 Token 统计格式

#### New Components
- `ToolOutputMessage.tsx` — 渲染 `function_call_output`（Codex）和 `tool_result`（Claude）的独立组件
- `AssistantMessage.tsx` 扩展支持 `reasoning` 和 `function_call` 块类型

### Fixed

#### Mac 搜索闪退（Critical）
- **根因**: `search.rs` 使用字节索引切片 UTF-8 字符串（`&text[..100]`、`text[start..end]`），遇到中文/emoji 时 panic
- **修复**: 全面替换为字符级安全操作 — `safe_truncate()` 使用 `chars().take(n)`，`extract_context()` 使用字符数组 + `windows()` 滑动窗口

#### Codex 会话消息数统计不准
- **根因**: `count_messages()` 使用 `contains("response_item") && contains("message")` 宽松匹配，如果函数返回内容中碰巧包含这些字面字符串会被误计
- **修复**: 改用精确匹配 `"type":"response_item"` 和 `"type":"message"` 的紧邻组合

#### Codex 会话黑屏
- **根因**: `MessagesPage.tsx` 使用 `location.pathname` 手动切片提取 filePath，对于 Codex 文件路径中含 `:` `\` 等特殊字符时 URL 编码/解码不一致导致前缀不匹配 → 空 filePath → 后端找不到文件
- **修复**: 改用 React Router 的 `params["*"]` 通配符参数，由框架负责解码

#### SessionsPage 重复解码
- `useParams()` 已自动解码 URL 参数，但代码又调了一次 `decodeURIComponent`，导致 `%25` 等被双重解码。移除多余的 decode 调用。

#### SearchPage 字段不匹配
- SearchPage 引用了旧的 `result.encodedName` 字段（统一模型中已不存在），改为使用 `result.projectId` 和 `result.filePath`

#### StatsPage 状态引用错误
- StatsPage 引用了已移除的 `stats`（旧 StatsCache），完全重写为使用 `tokenSummary`（统一 TokenUsageSummary），支持双数据源

### Changed

- 项目更名：Claude Memory Viewer → **AI Session Viewer**
- 应用标识符：`com.zuolan.claude-memory-viewer` → `com.zuolan.ai-session-viewer`
- 版本号：0.4.0 → **0.5.0**
- `watcher/fs_watcher.rs` 同时监听 `~/.claude/projects/` 和 `~/.codex/sessions/` 两个目录
- `terminal.rs` 根据 source 分别执行 `claude --resume` 或 `codex resume`
- 前端所有 API 调用和状态管理加入 `source` 参数
- `MessageThread.tsx` 支持三种角色路由：user → UserMessage，tool → ToolOutputMessage，其他 → AssistantMessage

---

## [0.4.0] - 2026-02-07

### Added

#### Session Deletion
- Delete individual sessions from the session list page
- Backend `delete_session` command removes the `.jsonl` file and updates `sessions-index.json`
- Trash icon button on each session card (visible on hover, alongside Resume)
- Confirmation dialog before deletion with loading state
- Session is removed from the local store immediately after successful deletion

---

## [0.3.0] - 2026-02-07

### Added

#### Scroll-to-Bottom Button
- Added a floating scroll-to-bottom button in the session message view for quickly jumping to the latest messages

---

## [0.2.0] - 2026-02-07

### Fixed

#### Resume Session — Terminal Lifetime
- **Critical**: Resumed terminals no longer get killed when the app exits
  - **Windows**: Replaced direct `cmd` spawn with `cmd /c start /d` — the `start` command launches a fully independent process owned by Windows shell, not by our app. The intermediate `cmd /c` exits immediately, breaking the parent-child link. `CREATE_NO_WINDOW` hides the brief intermediate cmd flash.
  - **Linux**: Added `process_group(0)` (calls `setsid`) to create an independent process session that survives parent exit.
  - **macOS**: Already independent (Terminal.app owns the process via AppleScript).

#### Linux Build
- Fixed `AsRef<OsStr>` type inference ambiguity caused by `glib` crate on Linux — removed unnecessary `.as_ref()` call in `Command::args`.
- Fixed `format!` temporary `String` lifetime issue — pre-bind formatted strings with `let` before referencing in array.

---

## [0.1.0] - 2026-02-07

First release of Claude Memory Viewer.

### Added

#### Project Browser
- Auto-scan `~/.claude/projects/` directory to discover all Claude Code projects
- Display project path, session count, and last active time
- Sort projects by most recently active

#### Session List
- Read Claude Code's `sessions-index.json` for instant loading
- Show session summary, first prompt preview, message count, Git branch, created/modified timestamps
- One-click Resume button to open `claude --resume {sessionId}` in system terminal

#### Message Detail
- Full conversation rendering with paginated loading (infinite scroll)
- **User messages** — plain text and tool result display
- **Assistant messages** — Markdown rendering with GFM support (tables, task lists, strikethrough)
- **Code blocks** — Syntax highlighting via Prism (oneDark theme), supporting 100+ languages
- **Thinking blocks** — Collapsible display of Claude's reasoning process
- **Tool calls** — Collapsible display of tool name, input parameters, and results
- Large content truncation (2000 chars) with expand option

#### Global Search
- Cross-project, cross-session full-text search
- Parallel scanning powered by Rayon (Rust)
- Case-insensitive matching with keyword highlighting
- Click results to navigate directly to the matching message

#### Token Statistics
- Read `stats-cache.json` for usage data
- Summary cards: total messages, sessions, tool calls, tokens
- Daily activity bar chart (messages + tool calls)
- Token usage trend area chart (input / output over time)
- Model usage distribution with progress bars

#### Resume Session (Cross-platform)
- **Windows** — Opens new CMD window via `cmd /c start`
- **macOS** — Uses AppleScript to open Terminal.app
- **Linux** — Auto-detects gnome-terminal / konsole / xfce4-terminal / xterm

#### Infrastructure
- Tauri v2 desktop app (Rust backend + React frontend)
- React 19 + TypeScript + Vite 6 + Tailwind CSS
- Zustand state management
- GitHub Actions CI (cargo check + clippy + tsc)
- GitHub Actions Release workflow for multi-platform builds (Windows / macOS Intel / macOS ARM / Linux)
- MIT License

### Technical Details

- **JSONL Parser**: Stream-based parsing with `BufReader` + line-level pre-filtering, skips `progress` and `file-history-snapshot` records for performance
- **Session Index**: Leverages Claude Code's built-in `sessions-index.json` for millisecond-level session list loading
- **Search**: Rayon parallel brute-force search across all JSONL files
- **Path Handling**: Cross-platform Claude home detection (`%USERPROFILE%\.claude` on Windows, `~/.claude` on Unix)

[1.6.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.6.0
[1.5.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.5.0
[1.4.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.4.0
[1.3.1]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.3.1
[1.3.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.3.0
[1.1.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.1.0
[1.0.1]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.0.1
[1.0.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.0.0
[0.8.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.8.0
[0.7.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.7.0
[0.6.1]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.6.1
[0.6.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.6.0
[0.5.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.5.0
[0.4.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.4.0
[0.3.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.3.0
[0.2.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.2.0
[0.1.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v0.1.0
