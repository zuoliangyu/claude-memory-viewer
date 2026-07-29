# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- 新增 Grok CLI 本地会话支持：读取 `$GROK_HOME/sessions/` 或 `~/.grok/sessions/` 下的 `summary.json` 与 `chat_history.jsonl`，支持项目/会话浏览、分页消息、全文搜索、导出、标签与别名、恢复、文件监听、单会话及项目删除和桌面端回收站恢复。

### Fixed

- Grok 与 Claude/Codex 统一走既有会话生命周期；排除系统提示和合成提醒，并隔离尚未支持 Grok 的 CLI 对话、Token 统计及 Provider 同步入口。

---

## [2.16.3] - 2026-07-11

### Added

- 「所有项目」页新增“刷新缓存”按钮，桌面端与 Web 端均可手动强制重建项目缓存；Codex 刷新会重新扫描完整 rollout 文件索引，修复原刷新链路仅返回旧缓存的问题。

### Fixed

- 修复 AI Session Viewer 关闭期间新增、修改或删除的 Codex rollout 未同步到项目列表的问题。Codex 磁盘索引现在会在每个进程首次载入时与 `~/.codex/sessions/` 做一次轻量对账，仅重读新增或变更文件；缓存版本升级到 8，首次启动会自动重建旧缓存，无需手动删除 `codex-list-cache.json`。

### Version

- 将工作区版本统一提升到 `2.16.3`，同步 `package.json`、`package-lock.json`、`Cargo.lock`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.16.2] - 2026-06-27

### Added

- **Codex 直连对话改为三层下钻 + 置顶**（新增 `src/utils/directChat.ts`、`src/components/project/DirectChatDatesPage.tsx`，改 `src/App.tsx`、`src/components/project/ProjectsPage.tsx`、`src/components/layout/Sidebar.tsx`、`src/components/session/SessionsPage.tsx`）：2.16.1 把每条直连对话按日期收成了 `<codex-direct>/DATE` 虚拟项目，但它们仍平铺在项目列表里、各占一格。本版本把这些日期桶在前端折叠成**一张「Codex 直连对话」聚合卡并固定在「所有项目」左上角第一位**，点进去是按日期归档的中间页（`/direct-chat`），再点某一天才进入该天的会话列表——形成「直连对话 → 日期 → 会话」三层结构。侧边栏项目列表同步折叠成单个「Codex 直连对话」入口。
  - 折叠/聚合逻辑抽到共享 `directChat.ts`（`collapseDirectBuckets` / `buildDirectGroup` 等），主网格与侧边栏共用一份；聚合卡为纯前端合成（id `<codex-direct-group>`），不落后端、不参与删除 / 改名 / 标签筛选 / 多选，避免被当真项目误操作。
  - 各日期桶仍是真实后端项目，日期中间页点进去复用现有 `/projects/:projectId` 会话列表（删除 / 克隆等操作在桶层级照常可用）；`SessionsPage` 返回按钮对直连桶返回 `/direct-chat`，保证下钻路径闭环。
  - 标签筛选激活时不折叠（聚合卡无标签，折叠会让筛选语义混乱）。

### Version

- 将工作区版本统一提升到 `2.16.2`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.16.1] - 2026-06-27

### Added

- **Codex 会话标题改用 Codex Desktop 线程名**（`crates/session-core/src/models/session.rs`、`crates/session-core/src/provider/codex.rs`、`crates/session-core/src/provider/claude.rs`、`crates/session-core/src/search.rs`，前端 `src/types/index.ts`、`src/components/session/SessionsPage.tsx`、`src/components/message/MessagesPage.tsx`、`src/components/cleanup/InvalidItemsPage.tsx`、`src/components/search/SearchPage.tsx`）：此前 Codex 会话标题只能退而取第一句 user prompt。本版本读取 `~/.codex/session_index.jsonl` 的 `thread_name`（Codex Desktop 自动生成的人类可读标题），作为 `SessionIndexEntry.thread_name` 新字段，显示优先级统一为 **alias > threadName > firstPrompt**；会话列表、消息页顶栏、分屏选择器、无效会话页与全局搜索结果一并接入。Claude 无此概念恒为空。实现上每次构建会话列表 / 搜索时读一次这个小文件（不进 `file_index` 缓存），标题随 Codex 更新保持新鲜。

- **Codex「直连对话」按日期归类为虚拟项目**（`crates/session-core/src/provider/codex.rs`、`src/components/layout/Sidebar.tsx`、`src/components/project/ProjectsPage.tsx`）：Codex Desktop 的「直接对话」会为每次对话新建临时工作目录 `…/Documents/Codex/<日期>/<slug>`，导致每条对话在项目列表里各成一个名字晦涩的「项目」（app、wo-2、j、re…）。本版本新增判别（`originator == "Codex Desktop"` 且 cwd 形如 `…/Codex/<日期>/…`），把它们收敛为按天分桶的 `<codex-direct>/DATE` 虚拟项目，显示为「Codex 直连对话 · 日期」。索引新增 `originator` 字段；`project_key_for` / `file_matches_project` 统一改为按桶 key 匹配，一次覆盖真实 cwd、未归属（unrooted）、直连三类。

- **Clone：非破坏式克隆 Codex 会话到其它 Provider**（`crates/session-core/src/provider_sync/{rollout.rs,sqlite_state.rs,service.rs,types.rs,mod.rs}`、`src-tauri/src/commands/provider_sync.rs` + `src-tauri/src/lib.rs`、`crates/session-web/src/routes/provider_sync.rs` + `crates/session-web/src/main.rs`、`src/components/session/CloneToProviderDialog.tsx`、`src/components/session/SessionsPage.tsx`）：`provider_sync` 此前只能就地改 provider，无法复制。新增 `run_clone`——复制 rollout 文件（新 UUID）+ 重写 session_meta 的 `model_provider` / `id`（浅克隆，正文逐字拷贝）+ 用 `SELECT *` 泛型读取并动态 `INSERT` 复制一行 `state_5.sqlite` 的 threads 记录，**原会话保留在旧 provider 下不动**。克隆前自动备份 `state_5.sqlite`（含 `-wal`/`-shm`）；含 `encrypted_content` 的会话给出「跨 provider/账号可显示但可能无法 resume/compact」告警。前端在 codex 会话悬浮操作新增「克隆到其他 Provider」入口 + 目标 provider 选择弹窗，成功后刷新列表。

### Changed

- Codex 列表磁盘缓存版本 `DISK_CACHE_VERSION` 由 5 提升到 7（新增 `thread_name` 读取与 `originator` 索引字段），首次启动本版本会触发一次性全量重扫。
- Sidebar / ProjectsPage 的虚拟项目 tooltip 文案改为对「未归属」与「直连对话」两类都准确的通用描述（具体含义由 displayPath 区分）。

### Version

- 将工作区版本统一提升到 `2.16.1`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.16.0] - 2026-06-09

### Added

- **Skills 浏览 / 查看 / 导入 / 删除**（新增 `crates/session-core/src/skills.rs`、`crates/session-core/src/models/skill.rs`、`src-tauri/src/commands/skills.rs`、`crates/session-web/src/routes/skills.rs`、`src/components/skills/`）：Claude Code 的 skills 此前在本应用中完全不可见。本版本新增独立 **Skills 页**（侧边栏入口）与会话页内嵌的**可折叠 Skills 面板**，统一查看三类来源——
  - **三类来源**：全局 `~/.claude/skills/`（**跟随符号链接**，正确收录 `lark-*` 这类软链）、插件 `~/.claude/plugins/{marketplaces,cache}/**/SKILL.md`（按 name 去重，避免市场克隆与缓存副本重复）、项目级 `<当前项目>/.claude/skills/`（取应用内正在浏览项目的真实路径）。frontmatter 用 `serde_yml` 只解析 `name` / `description`，忽略 `metadata` / `allowed-tools` 等其它键
  - **查看全文**：点击任一 skill 弹窗渲染完整 `SKILL.md`（复用既有 Markdown 渲染、自动剥离 frontmatter），可复制路径
  - **导入压缩包**：Standard 页「导入」支持选作用域（全局 / 当前项目）+ 选 `.zip` + 可勾「覆盖同名」，自动识别根级 `SKILL.md`（整包为一个 skill）或多个 `<子目录>/SKILL.md`（逐个导入）；用 `enclosed_name()` + 落地路径二次校验防 zip-slip。Tauri 走文件对话框取路径、Web 走原始字节 body（`POST /api/skills/import`，免 base64/multipart，body 上限提至 100MB）
  - **删除**（仅全局 / 项目，插件只读不动）：卡片悬停出现删除入口 + 二次确认。**符号链接安全**——软链只移除链接、保留目标（确认框明确提示）；真实目录递归永久删除并提示不可恢复
  - 后端删除按 `(scope, project_path, slug)` 反推路径，`slug` 必须为单段路径组件（防 `..` / 路径穿越），插件作用域直接拒绝；读取内容的路径校验拒绝 `..`、限定在 `~/.claude` 内或含 `.claude/skills` 段

### Version

- 将工作区版本统一提升到 `2.16.0`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.15.1] - 2026-06-07

### Fixed

- **Codex 已删除会话残留为「(无标题)」幽灵条目且无法删除**（`src-tauri/src/commands/sessions.rs`、`crates/session-web/src/routes/sessions.rs`、`crates/session-core/src/provider/codex.rs`）：在 Codex desktop 里归档/删除某个会话后，其 rollout 文件已从磁盘移除，但 ASV 内存索引里的条目仍在，渲染成「(无标题) · 0 条消息」；点删除时 `delete_session` 走 `validate_session_file → canonicalize` 在缺失文件上直接报错，前端 `catch` 只打日志、不移除条目，于是这条幽灵永远清不掉。本版本——
  - 让删除对「文件已不存在」幂等：文件在则照常移入回收站，文件已消失则跳过回收、清理元数据 + 失效缓存后返回成功，幽灵条目随之从列表移除（其它校验失败如非 `.jsonl` / 越权路径仍照常报错）；Tauri 与 Web 两条删除路径同步处理。
  - 列表自愈：`project_files` 与 `scan_projects_from_meta` 跳过索引中指向已消失文件的条目，刷新即自动剔除幽灵，无需手动删或重启。

- **全新 checkout 下 `cargo clippy --workspace` 因缺 `dist/` 编译失败**（新增 `crates/session-web/build.rs`）：`session-web` 用 rust-embed `#[folder = "../../dist"]` 内嵌前端，该 derive 在编译期要求目录存在，但 `dist/` 被 gitignore、只有构建前端才生成，导致未先建前端就跑 workspace 级 clippy / check 会失败（正是 CLAUDE.md 推荐的验证命令）。新增 `build.rs` 在编译前兜底创建空 `dist/` 目录；真实打包时前端产物已先写入，build.rs 自动跳过，对打包零影响。

### Version

- 将工作区版本统一提升到 `2.15.1`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.15.0] - 2026-06-03

### Added

- **会话导出为 JSON / Markdown / HTML**（新增 `crates/session-core/src/export.rs`、`src-tauri/src/commands/export.rs`、`crates/session-web/src/routes/export.rs`、`src/services/exportHelpers.ts`、`src/components/session/ExportFormatMenu.tsx`）：过去会话只能在应用内浏览，无法导出留档。本版本新增导出能力——
  - 核心渲染 `render_session` 复用既有 `parse_all_messages` 全量读取消息，路径仍经 `validate_session_file` 校验，覆盖全部内容块（text / thinking / tool_use / tool_result / Codex reasoning / function_call / function_call_output）；JSON 为结构化数据，Markdown 按角色标题 + 引用 + fenced 代码块排版，HTML 为单文件自包含（内嵌样式、内容全转义防 XSS）
  - 会话卡片悬停新增「导出」按钮，点选格式即可保存；Tauri 走系统保存对话框 + 后端写文件，Web 走浏览器 Blob 下载
  - **批量导出**：进入多选模式后一次导出多个会话（Tauri 选目录后每会话一个文件、文件名自动按别名/首条 Prompt 清洗去重，Web 逐个下载）

- **批量删除会话 / 批量删除项目**（`src/components/session/SessionsPage.tsx`、`src/components/project/ProjectsPage.tsx`）：列表右上角新增「选择」开关，进入多选模式后可勾选多个会话或项目，底部浮动操作条一键批量删除（移入回收站，可还原）。项目批量删除时 Claude 数据源可选「同时清理 CC 配置」。批量删除复用既有单删 API（已正确处理回收站 + 元数据 + 缓存），不新增删除端点。

- **Codex 项目删除**（`crates/session-core/src/provider/codex.rs` 新增 `delete_project`）：此前 `delete_project` 仅 Claude 支持，Codex 项目无法删除。现在 Codex 项目（含 `cwd` 为空、按日期合成的虚拟项目）也能删——遍历该项目的 rollout 文件逐个移入回收站、清理对应会话元数据、失效缓存。Tauri 命令与 Web 路由的 `delete_project` 均接通 codex 分支，单删入口（`ProjectActionsMenu` 的「删除会话数据」）也对 Codex 解锁。

- **冷启动扫描进度**（新增 `crates/session-core/src/scan_progress.rs`、`src-tauri/src/commands/progress.rs`、`crates/session-web/src/routes/progress.rs`、`src/components/common/ScanProgressView.tsx`）：初次启动需并行扫描成百上千个会话文件建缓存，过去这段时间界面只显示静态「加载中…」、用户易误以为卡死。现在三处扫描热点（Claude 项目扫描 / Claude 会话扫描 / Codex 索引构建）按完成数上报全局原子进度，前端加载态轮询 `get_scan_progress` 显示进度条 + `已扫描 / 总数` + 阶段文案 + 已用秒数；缓存命中时延迟 150ms 显示避免一闪而过。

### Changed

- **列表虚拟化：会话页 / 项目页只渲染可见行**（`src/components/session/SessionsPage.tsx`、`src/components/project/ProjectsPage.tsx`，引入既有依赖 `@tanstack/react-virtual`）：之前会话/项目列表未虚拟化，几百张卡片全量挂在 DOM 里，进入/退出多选模式时整列表同步重渲染 + 操作按钮组批量重排，长列表下会明显卡顿甚至卡死。本版本两页都改造为内部滚动容器 + 虚拟化——会话页单列纵向虚拟化（`measureElement` 动态测量变高卡片），项目页按容器宽度自适应列数（1~3）后对「行」做虚拟化。配合把 `date-fns` 日期格式化预计算缓存（`useMemo`，仅列表变化时重算），切换多选模式与滚动均为恒定开销、不再随数据量增长。页头与标签栏改为固定不滚动（与逐请求账单页一致）。

- **冷启动 CPU 限流：rayon 全局线程池留一核给 UI**（`crates/session-core/src/scan_progress.rs` 的 `configure_rayon_pool`，在 `src-tauri/src/lib.rs` 与 `crates/session-web/src/main.rs` 启动时调用）：冷启动并行扫描默认吃满全部 CPU 核，导致 webview 主线程被饿死、界面（甚至整机）卡顿、连进度条都画不动。现在把 rayon 全局线程池限制为「核数 − 1」（至少 1），扫描时 UI 始终可响应，代价是扫描略微变慢。

### Version

- 将工作区版本统一提升到 `2.15.0`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.14.4] - 2026-06-01

### Added

- **消息「跳到百分比」导航**（`src/components/message/MessagesPage.tsx` + 新增 `JumpToPercentControl.tsx` / `SessionPositionRail.tsx`）：长会话里要看中间/前面内容，过去只能点"加载全部"（串行翻几千条、慢）或一路滚动（全量加载后 DOM 几千节点、卡）。本版本新增两个跳转入口——顶部 `0/25/50/75/100%` 预设按钮 + 数字输入框，右侧一条可点击/拖动的竖直位置滑条（实时反映当前阅读位置）。两者都复用既有的 `jumpToMessageIndex` 窗口化加载，只取目标位置 ±15 条的一小段（走 `getMessagesRange`），所以加载快、DOM 小、滚动不卡。仅在 `messagesTotal > 60` 且非 Thread/搜索/分屏视图时显示。

### Changed

- **会话首开提速：末尾窗口延迟转换**（`crates/session-core/src/parser/jsonl.rs`）：打开会话只需末尾 30 条，但 `parse_tail_messages` 过去会把整个 JSONL 逐条 `convert_content`（对每个 tool_use 做 `to_string_pretty`、对每个 tool_result 拼大字符串）后只留末尾 ~120 条。改为只缓存末尾窗口的 `RawRecord`、仅对该窗口做转换；新增零分配的 `record_is_displayable` 镜像 `convert_content` 的非空规则来计数，`total` 与全量解析完全一致；并加子串闸门跳过非 user/assistant 行的 `from_str`。

- **ASV 冷启动提速：缓存按路径增量失效**（`src-tauri/src/watcher/fs_watcher.rs`、`crates/session-web/src/ws.rs`、`crates/session-core/src/provider/{claude,codex}.rs`）：fs-watcher 过去在任何文件变动时整份清空列表缓存，导致活跃用户每次打开 ASV 都要对所有项目的每个会话文件做全量扫描。现在按改动路径增量更新——
  - Claude `invalidate_paths`：改动全是 `.jsonl` 且项目会话列表已缓存时，只 `scan_single_session_file` 重扫那几个文件并 splice 进缓存（`enrich_with_index` + 重排序），其余情况回退到项目级 / 全量失效；
  - Codex `invalidate_paths`：只对变动文件重读 meta、更新内存索引、删受影响项目缓存，`projects=None` 做纯内存重聚合，不再全树遍历。

- **Codex 列表大改造：并行构建可复用 file index**（`crates/session-core/src/provider/codex.rs`）：之前 `scan_projects_from_meta`（列项目）是串行逐文件读 meta，`get_sessions`/`get_invalid_sessions`（点项目）会再走一遍整棵 `年/月/日` 目录树并重开每个文件 meta。新增缓存的 `file_index`（`rayon` 并行一次性扫描 path + session_meta + 时间戳），项目列表纯内存聚合、会话列表只扫该项目的少数文件。`DISK_CACHE_VERSION` `4 → 5`。

- **前端首屏瘦身**（`src/stores/appStore.ts`、`src/components/project/ProjectsPage.tsx`、`src/components/layout/Sidebar.tsx`）：`loadProjects` 加按 source 的 in-flight 去重，避免 Sidebar + ProjectsPage 冷缓存下并发触发两次全盘扫描；`detectCli()` / `loadCrossProjectTags()` 推迟到首帧之后，移出首屏关键路径。

### Version

- 将工作区版本统一提升到 `2.14.4`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.14.3] - 2026-05-27

### Fixed

- **"加载全部"进度循环波动**（`src/stores/appStore.ts`）：用户反馈在 Rocky Web 模式打开长会话（6214 条消息）点"加载全部"，进度条 `已加载 xx / 6214` 在小数和大数之间反复循环、永远不停。根因是 `refreshInBackground` 的尾部静默刷新分支与 v2.12 引入的渐进式窗口加载冲突——`handleLoadAll` 把 `loadedStart` 一路前移时 `loadedEnd` 始终 = `messagesTotal`，所以 `atTail` 守卫一直为真；Rocky 上 inotify 频繁触发 `fs-change`，每次 debounce 后都把整个 `messages` 数组覆盖回末尾 30 条，把 `loadedStart` 重置回 `total - 30`，外层循环看到 `messagesHasMore=true` 又从头爬，进度数字就在两个值之间循环。本版本：
  - 静默刷新前置 `messagesLoading` 守卫：用户正在跑"加载全部"循环时直接 skip
  - 文件总数没变（`result.total === after.messagesTotal`）：完全不动 `messages`，避免擦掉用户上翻的历史
  - 用户已上翻超出默认尾部页（`loadedStart < total - PAGE_SIZE`）：只更新 `messagesTotal` 和 `messagesHasNewer` 标记，不替换 `messages` 数组
  - 仅当窗口本来就只覆盖最后一页 + 文件确实新增了消息时，才整段替换（这是续聊追加的常规场景，行为不变）
  - 该 bug 在 Windows / macOS 桌面端也存在但不易复现——Linux inotify 比 Windows ReadDirectoryChangesW / macOS FSEvents 更敏感，加上 v2.14.1 之后 `sessions-index.json` 写入更勤，触发频率被放大才让 Rocky 用户碰上

### Version

- 将工作区版本统一提升到 `2.14.3`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.14.2] - 2026-05-24

### Added

- **Codex 未归属会话按日期建虚拟项目**（`crates/session-core/src/provider/codex.rs`）：之前 `cwd` 为空的 codex session 在 `scan_projects_from_meta` 里被一行 `continue` 直接丢弃——既不进项目列表也不进任何会话视图，等于在 UI 里彻底隐身。本版本把这类会话按 rollout 文件路径上的日期（`~/.codex/sessions/YYYY/MM/DD/`）合成虚拟项目：
  - 虚拟 ID 采用 sentinel 格式 `<codex-unrooted>/YYYY-MM-DD`，`<>` 在 Windows 路径里非法，与真实 cwd 不可能碰撞；提取不出日期的退化为 `<codex-unrooted>/unknown`
  - `ProjectEntry` 新增 `is_virtual: bool`（serde default，兼容旧缓存）；侧栏和项目页用 `FolderClock` 图标 + 灰色"未归属 · YYYY-MM-DD"文案区分
  - `refresh_sessions_cache` / `get_invalid_sessions` / `collect_requests` 都新增「识别虚拟 ID → 按 cwd 为空 + 文件日期匹配」的过滤分支，会话列表与项目消费榜均能正确归桶
  - 「清理无效项」页面**不**把虚拟项目当作"路径不存在"项目展示，避免误删——仅当虚拟项目内部确有 empty/corrupt 会话时才会出现在该页
  - `DISK_CACHE_VERSION` 由 `3` bump 到 `4`，让用户升级后首次启动即可看到这些新合成的虚拟项目，无需手动 refresh

### Version

- 将工作区版本统一提升到 `2.14.2`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.14.1] - 2026-05-23

### Added

- **损坏会话可见化 + 独立分组**：之前如果某个 `.jsonl` 中部出现 NUL 字节或解析失败的行（最常见是 CC 异常退出留下的稀疏空洞），整个文件会被 `scan_session_file_once` 标记 `is_corrupt` 后**静默丢弃**——既不出现在主列表，也不进"空会话清理"，用户完全看不到这文件的存在。本版本：
  - 新增 `SessionStatus` 三态枚举（`Valid` / `Empty` / `Corrupt`），每条 `SessionIndexEntry` 带 `status` 字段
  - **SessionsPage 顶部新增黄色横幅**：`本项目有 N 个会话因文件损坏被隐藏`，点击跳到 `/cleanup`
  - **InvalidItemsPage 摘要卡拆分**："无效会话"→"空会话 + 损坏会话"两张卡，损坏条目带 amber `AlertTriangle` 图标 + 「文件损坏，部分可读」徽章 + 故障原因描述
  - "查看详情"按钮对损坏会话同样可用——`parse_all_messages` 现有 `Err(_) => continue` 已经天然容错单行解析失败，损坏文件直接能开，坏行被静默跳过
  - 删除路径完全复用现有 `recyclebin::move_to_recyclebin`，桌面端走回收站、Web 端永久删
- **StatsPage 项目花费榜显示短名 + 数据覆盖范围提示**（`src/components/stats/StatsPage.tsx`）：
  - Top 10 Y 轴从全路径改为仅项目名（路径最后一段），完整路径移至 Tooltip 的 `labelFormatter`
  - 时间筛选行右上新增数据范围胶囊「数据覆盖 YYYY-MM-DD ~ YYYY-MM-DD（共 N 天）」，解释为何"全部"和"最近 30 天"看起来重叠——不是筛选 bug，是源数据本来就只覆盖那么多天

### Changed

- **Claude 会话列表性能大幅优化**（`crates/session-core/src/provider/claude.rs`）：用户反馈"点侧边栏黄色项目直接卡死"，profiling 发现单次点击会触发两路完整扫描（每个 `.jsonl` 读两遍）+ 项目内文件全程串行。本版本：
  - **rayon 并行扫描**：`scan_project_dir_all` 用 `into_par_iter` 按 CPU 核数并行调 `scan_session_file_once`，对 76 文件 / 230MB 的项目，8 核机器上约 6-8 倍加速
  - **统一缓存**：`get_sessions` 和 `get_invalid_sessions` 共享同一份"全分类"缓存（v3），`get_invalid_sessions` 命中缓存直接 filter 不再读盘
  - **API 契约调整**：`get_sessions` 现在返回 **全分类** `Vec<SessionIndexEntry>`（Valid + Empty + Corrupt），前端按 `status` 自己 filter；bump `CACHE_VERSION` 2 → 3 让老缓存被丢弃重建
  - **`scan_session_file_once` 拆分**：抽出 `scan_single_session_file(PathBuf) -> Option<SessionIndexEntry>`，便于 par_iter 调度
- **前端去重 SessionsPage 双 RPC**（`src/components/session/SessionsPage.tsx` + `src/stores/appStore.ts`）：之前 `SessionsPage` 的 useEffect 同时打 `selectProject` + `loadInvalidSessions` 两个 RPC，冷缓存下两个调用会**各自启动一次完整扫描**——这是"点侧边栏卡死"的直接根因。本版本：
  - `appStore` 新增 `invalidSessions` 字段，`selectProject` 一次 RPC 同时填 `sessions`（Valid）+ `invalidSessions`（Empty + Corrupt）
  - `SessionsPage` 删掉本地 `invalidSessions` state 和 `loadInvalidSessions` 函数，直接读 store
  - `deleteSession` / `deleteProject` / `setSource` / `refreshInBackground` 都同步维护 `invalidSessions`
- **InvalidItemsPage 改流式渲染**（`src/components/cleanup/InvalidItemsPage.tsx`）：之前需要等全部 95 个项目串行扫完才显示，体感分钟级白屏。本版本：
  - 项目列表一到立刻渲染"路径不存在"的项目骨架
  - 顶部进度条「正在扫描会话异常项（42 / 95）· 已发现 12 个问题项目」+ 百分比进度条
  - 每个项目扫完独立 push 进 groups，按优先级排序插入正确位置，**用户能立即和"已扫完"的项目交互**
  - 新发现的问题项目默认展开
  - `reloadEpochRef` 防止并发刷新串结果

### Version

- 将工作区版本统一提升到 `2.14.1`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.14.0] - 2026-05-23

### Added

- **逐请求账单 / Per-Request Billing**：原来 Token 统计只能看到「每日合计」，看不到单条请求的开销。本版本把 JSONL 里每条 assistant 消息的 `usage` 完整保留，新增独立账单视图 `/stats/requests`：
  - 表格列：时间 / 项目 / 模型 / 会话 / `input` / `cache 读` / `cache 写` / `output` / 耗时 / 花费
  - 使用 `@tanstack/react-virtual` 虚拟滚动，10 万条记录也不卡
  - 项目 / 模型 / 起止日期可筛，URL 参数保留筛选状态便于分享
  - 点击行直接跳到对应消息（通过 `message_uuid` 锚点）
  - 「耗时」= 该 assistant 消息时间戳与上一条 user 消息时间戳之差，无需后端计时
- **会话级账单徽标（MessagesPage）**：消息详情页顶栏出现绿色 chip「本会话累计 $X · N req」，点击弹 Modal 展示每条请求的迷你账单，「复制 Markdown」一键导出为表格——可以直接贴到团队周报 / 报销单。
- **项目花费排行 Top10（StatsPage）**：水平柱状图按 cost 降序展示项目花费，**点击柱形跳到该项目的逐请求账单**。
- **缓存命中率走势图**：按模型分线显示 `cache_read / (input + cache_read + cache_creation)` 时间曲线，60% 红色参考线。Legend 改为**可点选 chip**——多条线时点击模型 chip 切换显示 / 隐藏，避免重叠的曲线难以区分。
- **单日筛选自动切换按小时聚合**：之前选「今天」只有一个数据点（无法看趋势）。现在 `start === end` 时前端会拉当日的逐请求数据，按本地时区 24 小时桶重算 Token 用量 / 花费 / 缓存命中率三张图，标题显示「按小时」徽标。
- **模型价格表 + cost 计算**（`session-core/models/pricing.rs`）：内置 Claude 3.x/4.x、GPT-5/4.x/4o、o1/o3/o4 主流模型价格表，前缀匹配兼容带日期后缀的型号（如 `claude-sonnet-4-5-20250929`）。Anthropic 模型应用 cache 1.25× / 0.10× 倍率。未知模型 cost = 0，避免误导。
- **缓存读 / 写 Token 分项**：原来 `cache_read_input_tokens` 和 `cache_creation_input_tokens` 被合进 `input_tokens` 聚合后丢失，本版本把这两个流单独保留，新增「缓存读取 Token」「缓存写入 Token」「缓存命中率」三个 StatCard。

### Changed

- **Stats 缓存进程内常驻 + Singleflight**（`session-core/src/stats.rs`）：原实现每次 `get_stats` 调用都要 `fs::read_to_string(stats-cache.json)` + `serde_json::from_str`、计算、`to_string` + `fs::write` 整个 57MB JSON，并且 StatsPage 同时发起 `getStats` + `getProjectCosts` → 两次完整 IO。重写为：
  - `OnceLock<Mutex<CacheState>>` 持有进程内 cache，首次访问 load 一次，之后全程内存
  - 750ms throttle fast-path：并发调用合并为一次扫描，第二个调用阻塞 < 1ms 直接读 in-memory
  - 异步落盘 `std::thread::spawn(snapshot.clone())`：RPC 响应不等磁盘
  - `BufReader` / `BufWriter` 流式 IO 替代 read_to_string / to_string，省内存峰值 + ~30% 时间
- **Stats cache schema 重构**（CACHE_VERSION 3 → 4）：`FileStat.requests` 改用内部 `CompactRecord` 类型——字段名缩为单字符 (`t` / `m` / `i` / `o` / `cr` / `cw` / `c` / `d` / `s` / `u`)，并去掉 `source` / `file_path` / `project_id` 三个可从 `FileStat` key 推断的冗余字段。在用户 57MB cache 上预期降至约 20-25MB，反序列化时间相应减半。**首次升级会一次性重扫所有 JSONL**（屏幕显示「正在建立统计索引」），之后所有操作秒回。
- **`scan_file` 双重 substring 预筛**：`assistant` 行额外要求包含 `"usage"` 才进 serde 解析，跳过明显不含 token 数据的助手回复，减少 ~40% serde 调用。
- **`TokenUsageSummary` / `DailyTokenEntry` 扩展字段**：分别新增 `total_cache_read_tokens` / `total_cache_creation_tokens` / `total_cost_usd` / `cost_by_model`，以及 `cache_read_tokens` / `cache_creation_tokens` / `cost_usd` / `cache_hit_ratio_by_model`。
- **StatsPage 主表行 X 轴 dataKey 改用统一 `label`**：按日 / 按小时模式下都用同一 key，避免 chart prop 在两种模式间切换。

### API

- **新增 Tauri 命令**：`get_request_log(source, projectId?, sessionId?, startDate?, endDate?, model?, page?, pageSize?)` / `get_project_costs(source)` / `get_session_cost(source, filePath)`
- **新增 REST 路由**：`GET /api/stats/requests` / `GET /api/stats/projects` / `GET /api/stats/session`
- **新增前端类型**：`RequestRecord` / `RequestLogPage` / `ProjectCostEntry` / `SessionCostSummary` / `RequestLogFilter`（`src/types/index.ts`）

### Codex 数据源说明

Codex JSONL 的 `token_count` 事件只暴露累计 `total_token_usage`，不区分 cache_read / cache_creation。本版本对 Codex 做了相应的兼容处理：
- `extract_token_events` 从相邻 `token_count` 事件计算 delta 得到每轮 usage（也兼容 `last_token_usage` 字段）
- `cache_read_tokens` / `cache_creation_tokens` 字段在 Codex records 上恒为 0
- 缓存命中率走势图在 Codex 数据源下不显示曲线（数据不存在）
- 项目 ID 用 session_meta 的 `cwd` 字段，display name 取路径末段

### Version

- 将工作区版本统一提升到 `2.14.0`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.13.0] - 2026-05-20

### Added

- **Codex Provider 同步工具**（仅 Codex 数据源）：切换到 Codex 后侧栏底部出现「Provider 同步」入口，专门处理"在 codex 里改了 `model_provider` 之后，老 rollout / SQLite 元数据还指向旧 provider，导致 Codex Desktop 和 `/resume` 看不见历史会话"这个问题。参考 [Dailin521/codex-provider-sync](https://github.com/Dailin521/codex-provider-sync) 的核心做法，在本应用里做成 GUI。
  - **状态总览**：扫描 `~/.codex/sessions/` + `~/.codex/archived_sessions/` + `state_5.sqlite`，按 provider 分组展示分布、给出不一致计数；如果旧 provider 的会话里含 `encrypted_content`，单独警告（同步后能看见但「继续对话」可能报 `invalid_encrypted_content`）。
  - **同步**：把所有不匹配当前 `config.toml` 中 `model_provider` 的 rollout 首行 `payload.model_provider` 改写过来，同步 `state_5.sqlite` 的 `threads` 表三列（`model_provider` / `has_user_event` / `cwd`，单事务），并规范化 `.codex-global-state.json` 中的 Windows verbatim 路径（`\\?\C:\…` → `C:\…`）。被锁的 rollout 不让整体失败，会单独列在「跳过」清单里。
  - **切换**：选个 provider 一键改写 `config.toml` 顶层 `model_provider` 并执行同步。可从已配置的 `[model_providers.*]` 列表里挑，也支持自定义 id。
  - **自动备份 + 恢复**：每次同步前完整拷贝 `state_5.sqlite`（含 `-shm`/`-wal`）、`config.toml`、`.codex-global-state.json` 到 `~/.codex/backups_state/provider-sync/<UTC时间戳>/`，并把被改 rollout 的首行原文 + 原 mtime 写入 `session-meta-backup.json`。备份列表 UI 里可勾选粒度恢复（config / db / sessions / global-state 各自独立开关）。默认保留最近 5 份，超出可一键清理。

### Changed

- **后端依赖**：`session-core` 新增 `rusqlite`（bundled feature，编译自带 SQLite 静态库）和 `filetime`。bundled 模式首次编译会多 30–60 秒，但运行时无系统 SQLite 依赖，跨发行版可直接跑。
- **API 表面**：
  - Tauri 命令：`provider_sync_status` / `provider_sync_run` / `provider_sync_switch` / `provider_sync_restore` / `provider_sync_prune`
  - REST：`GET /api/provider-sync/status` + `POST /api/provider-sync/{sync,switch,restore,prune}`

### Version

- 将工作区版本统一提升到 `2.13.0`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.12.5] - 2026-05-08

### Fixed

- **Windows 启动 / 打开 Chat 页时闪 cmd 黑框**：主进程一直有 `#![windows_subsystem = "windows"]` 拦 console，但 `session-core` / `src-tauri` 里几处 spawn 子进程拿默认创建标志，从无 console 的父进程下来会临时新建一个 cmd 窗口闪一下。本版本三处 spawn 全部补 `CREATE_NO_WINDOW (0x0800_0000)`：
  - `session-core::cli::which_binary` 的 Windows 分支 `where <name>` —— 探测 CLI 路径每次闪。
  - `session-core::cli::get_cli_version` 的 `<cli> --version` —— Claude 和 Codex 各探测一次，连闪两下。
  - `src-tauri::commands::chat::kill_process` 的 `taskkill /PID … /T /F` —— 取消 / 关闭聊天流时弹。
  在 `cli.rs` 加 `hide_console(&mut Command)` cfg-gated helper 收口（Unix 上是 no-op），避免把同样的 `creation_flags` 模板再抄第四遍。

### Version

- 将工作区版本统一提升到 `2.12.5`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.12.4] - 2026-05-08

### Fixed

- **"加载全部"在新渐进式管线下死循环卡 30 条**：v2.12.3 把 `loadMoreMessages` 改成 range API 之后，外层 `handleLoadAll` 的循环依赖 `messagesHasMore` 退出。一旦 `loadMoreMessages` 走到 catch 分支（接口异常 / 部署期前后端不一致 / 后端返回空切片），它只把 `messagesLoading` 置回 false，`messagesHasMore` 保留 true → 外层 `for (i < 500)` 循环每一轮都重新调一次，但 `loadedStart` 不动，UI 永远卡在初始的 30 条。本版本三处修：
  - `appStore.loadMoreMessages` / `loadNewerMessages` 拿到响应后做"无进展"判断：若 `result.messages` 为空、或 `result.start` 没真的小于当前 `loadedStart`（反向同理），强制把对应方向的 `messagesHasMore` / `messagesHasNewer` 关掉，并 `console.warn` 出 `{ requestStart, requestEnd, resultStart, resultLen }`，方便用户从控制台直接看出后端实际返了什么。
  - `loadMoreMessages` / `loadNewerMessages` 的 catch 分支也把 `messagesHasMore` / `messagesHasNewer` 关掉 — 失败要让 UI 诚实，不能靠错误状态自旋。
  - `MessagesPage.handleLoadAll` 加 no-progress 守卫：每轮调 `loadMoreMessages` 前后对比 `loadedStart`，没推进就 `console.error` 后 break，不再傻跑到 i=500。
- **`loadMoreMessages` / `loadNewerMessages` 的 stale-source 早 return 漏重置 `messagesLoading`**：原代码在中途切换数据源 / 文件时直接 return，`messagesLoading` 保留 true，下一次进入旧路径时仍然被守卫拦下。本版本所有早 return 都补上 `messagesLoading: false`。

### Version

- 将工作区版本统一提升到 `2.12.4`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.12.3] - 2026-05-08

### Added

- **消息渐进式窗口加载**：进会话默认只加载尾部 30 条（之前 100 条），首屏从 ~1-2s 缩到亚秒级。上滑近顶 200px 自动拉更老的，下滑近底 200px 且窗口未到尾时自动拉更新的。
- **后端 range API**：`session-core` 新增 `parse_messages_range(path, start, end)`（Claude / Codex），命中已缓存范围直接切片返回，否则全文件解析后写缓存复用。同步暴露：
  - Tauri 命令 `get_messages_range`
  - Web 路由 `GET /api/messages/range?source=&filePath=&start=&end=`
- **`appStore` 跳转 action**：新增 `jumpToMessageIndex(targetIndex)`，把窗口替换为 `[target-15, target+16)`。为后续 TOC 跨窗口跳转打底（本版本 TOC 仍只展示已加载窗口内的提问，跨窗口跳转留作 Phase B）。

### Changed

- **`appStore` 状态字段**：删除 `messagesPage`，改为 `loadedStart` / `loadedEnd` / `messagesHasNewer` 三字段维护窗口边界。`messagesHasMore` 语义保留（= `loadedStart > 0`，更老有内容可载）。`selectSession` / `reloadLatestMessages` / `refreshInBackground` / `selectProject` / `deleteProject` / `setSource` 全部对齐到新字段。
- **`MessagesPage` 滚动加载双向触发**：原来只在近顶时调 `loadMoreMessages`（向前扩窗），现在新增 `requestNewerMessages` 在近底且 `loadedEnd < total` 时触发（向后扩窗），让用户从中段往后滚也能继续加载。

### Fixed

- **全局搜索页首次进入卡 5s**：`get_all_cross_project_tags` 的 Claude 分支之前顺序遍历 `~/.claude/projects/` 下每个目录读 `.session-viewer-meta.json`，N 个项目就是 N 次串行磁盘 IO。改用 rayon `into_par_iter`，并行读 metadata 文件。8 核机器 + 50 个项目下从 5s 量级回到亚秒级。Codex 分支只有一个全局 metadata 文件，无需并行。

### Version

- 将工作区版本统一提升到 `2.12.3`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.12.2] - 2026-05-07

### Fixed

- **NSIS 安装版被错认成 portable，应用内自动更新失效**：v2.12.0 给 `get_install_type()` 加了一道"必须同时存在 NSIS 注册表 Uninstall 键"的校验，硬编码用 bundle identifier (`com.zuolan.ai-session-viewer`) 作为子键名。但 Tauri NSIS 模板在不同版本里写注册表用的可能是 `MAINBINARYNAME` / `PRODUCTNAME` 等，这个校验**对真实通过 NSIS 安装的用户全员误判**为 portable，应用内更新流程直接被踢去"打开 GitHub Release 页面手动下载"。本版本**回退到只看 `uninstall.exe` 是否存在**，恢复到 v2.11 及之前的行为。原本想防御的"伪造 uninstall.exe 骗过去走 NSIS 更新管线"是非常小众的本地攻击场景，不值得让所有正版用户失去自动更新。
- **移除 `winreg` 依赖**：v2.12.0 为上面那道校验在 `src-tauri/Cargo.toml` 加的 Windows-only `winreg = "0.55"` 也一并清理掉。

### Version

- 将工作区版本统一提升到 `2.12.2`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.12.1] - 2026-05-07

### Fixed

- **Codex 项目列表升级后仍漏会话**：v2.12.0 把 `extract_session_meta` 的扫描范围从 5 行扩到 50 行、把 `is_interactive` 改成黑名单，但旧版本扫出来的结果会持久化到 `dirs::config_dir()/ai-session-viewer/codex-list-cache.json`。`DISK_CACHE_VERSION` 没动 → 新版启动时检测版本一致就直接读老缓存 → 修复对**已经升级的用户不生效**。本版本把 `DISK_CACHE_VERSION` 从 `2` 提升到 `3`，触发首次启动自动重扫；旧缓存被丢弃重建，DSP 这种之前漏掉的项目会自动出现在项目列表里，不用再手删缓存文件或等 10 分钟后台刷新。

### Version

- 将工作区版本统一提升到 `2.12.1`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.12.0] - 2026-05-07

### Security

- **路径穿越加固**：新增 `session_core::paths::validate_session_file` 共用 helper，把"路径必须 canonicalize 到 `~/.claude/projects/` 或 `~/.codex/sessions/` 之内 + 必须是 `.jsonl` + 必须符合源对应的目录层级"逻辑收敛到 session-core；Tauri 的 `delete_session` / `update_session_meta` 改成先走它，session-web 的 `resolve_session_file_path` 改成委托过来。`metadata::rename_chat_session` 入口新增 `validate_session_id`，拒绝 `..`、路径分隔符、空字符等会让 `<encoded-project>/<session_id>.jsonl` 逃出 project_dir 的 session_id。
- **终端 shell 注入修复**：`commands/terminal.rs` 之前把 `session_id` / `project_path` 直接拼进 `bash -c '…'`、`cmd /c start /d …`、`osascript "do script \"cd '…' && …\""`，恶意 session 文件可注入命令。修复：入口校验 `session_id`；macOS / Linux 用新 `posix_single_quote` 把路径里的 `'` 转义为 `'\''` 并对 AppleScript 外层做 `\` / `"` 转义；Windows 改用 `current_dir()` + `CREATE_NEW_CONSOLE` 直接 spawn `cmd /k` / `powershell -NoExit`，不再走 `cmd /c start`，路径里的 `&` / `|` / `^` 不再会被 cmd.exe 二次解析。
- **WebSocket 凭据不再走 query string**：原本 `?token=<long-lived-secret>` 会落到反代 access log。新增 `POST /api/auth/ws-ticket`（需 Bearer 认证）签发 30 秒一次性票据，`require_ws_auth` 校验票据后即消费；`buildAuthenticatedWebSocketUrl` 现在先去拿 ticket 再拼到 `?ticket=…`。Bearer header 路径保留供非浏览器客户端使用。

### Fixed

- **Web 多 pane 流式串消息 / 串 cancel**：`startChat` / `continueChat` 之前共享一个全局 `chatWsResolve`，并发请求会互相覆盖；`output` / `error` / `complete` 帧没有顶层 `sessionId`，分屏会互相收对方的事件；`cancelChat` 直接发 `{action:"cancel"}` 全局广播。修复：服务端给所有帧打 `sessionId`，客户端用 `pendingChatStarts: Map<routingId, {resolve,reject}>` 路由，cancel 携带 sessionId，后端按 sessionId 维护 per-session `watch::Sender` / `CodexTurnState`。
- **Tauri 新会话 pending → real session_id 切换丢流**：Claude system_init 把 pane.sessionId 改成真实 id，hook 重新订阅新事件名，但后端仍在 pending 通道发事件 → 后续 stdout 全部掉地上。引入 `pane.streamId`（turn 内绝不变）作为路由键，`sessionId` 仅作展示/续聊用。Codex Tauri 端同步把 `chat-output:{thread_id}` 改成稳定的 `chat-output:{event_id}`。
- **Codex `CodexAppServer` 单 runtime 误杀其它 pane**：之前 `inner: Option<Runtime>`，凭据指纹一不一样就 `fail_pending` + 替换 runtime，会让正在跑的别的 pane 全部失败。改为 `LruCache<fingerprint, Arc<Runtime>>`（容量 4），按指纹隔离；超量时 LRU 淘汰最旧的，关闭其 stdin 让 codex CLI 自然退出。`subscribers` 同步从 `HashMap<thread, Sender>` 升到 `HashMap<thread, Vec<Sender>>` + `dispatch_and_prune`，两个 pane 同时 resume 同一 thread_id 不再互相顶掉。
- **Tauri `cancel_chat` 错用磁盘凭据**：之前 `resolve_credentials("codex", None, None)` 重新从磁盘 config 取 creds，跟 turn 启动时的 override 凭据不一致就会让 `interrupt_turn` 触发 runtime 替换、连带杀别的 turn。`codex_sessions` 现在存 `CodexSessionEntry { thread_id, credentials }`，cancel 用启动时缓存的凭据。
- **Tauri `codex_sessions` 永久泄漏**：之前只插入不删除。`stream_codex_notifications` 末尾、`cancel_chat` 路径、`turn/start` 失败分支都补上 `drop_codex_session_entries` 清理；`turn/start` 失败时还会主动 emit `chat-complete` 让前端从 streaming 状态退出。
- **`oneshot::Receiver` drop 误中断成功 turn**（自查发现的回归）：上一版引入的 abort 机制用 oneshot，start_turn 成功路径的 `drop(abort_tx)` 会让 `select!` 的 `_ = &mut abort_rx` 解析为 `Err(RecvError)` 触发 — **每次 Codex 成功 turn 都会被立刻误中断**。换成 `Arc<tokio::sync::Notify>` + `notify_one`（drop 不触发 waiter）。
- **切换数据源时未取消 in-flight 流**：从 Claude 切到 Codex（或反向）时旧流事件还在到达，但前端 parser 已切换 → 整段对话被错误解析成乱码且 `isStreaming` 不会被清。`setPaneSource` 现在检测 streaming 时先 `cancelPane`，再用 `clearPane` 的方式重置消息/sessionId/streamId/error。
- **`asv-auth-required` 没有监听器**：事件 dispatch 了但全代码没人监听，401 时 UI 静默无反应。新增 `<AuthGate />` 全局组件，弹窗收集 token 写入 localStorage；新增 `withAuthRetry` + `awaitAuthRestoration`，所有 fetch wrapper（apiFetch / apiDelete / apiPut / apiPost / probeWebSocketAuth / fetchWebSocketTicket / permanentlyDeleteRecycledItem）在 401 后会等用户填完 token 自动重试一次，多个并发 401 共享同一 promise。
- **Codex 项目列表漏会话（搜得到但项目里没有）**：`extract_session_meta` 之前只读 JSONL 前 5 行找 `session_meta`，新版本 codex / app-server 在 session_meta 之前夹的几行 housekeeping 会让它返回 None；`is_interactive` 又是白名单（只认 `cli` / `vscode`），未知 source 直接被隐藏。改：扫描行数提到 50 + 剥离 UTF-8 BOM；`is_interactive` 改成黑名单（只屏蔽明确的 `exec` / `mcp` / subagent 对象），未知 source 默认显示，跟全局搜索的"全索引"行为对齐。
- **Web `loadAllTags` / `loadCrossProjectTags` 旧请求结果污染新视图**：source 切换瞬间 inflight 的旧请求回来后会把旧 source 的 tag 写进新 source 的 view。两个 loader 都加 source/project 旧值快照对比，不一致直接丢弃结果。
- **Web 文件监听线程不会重启**：`std::thread::spawn` 出去的 watcher 一旦失败/通道关闭就静默退出，整个进程后续没文件变更推送。改成监督循环（5s 退避后重启）。
- **Web `skipPermissions` 设置被忽略**：之前不论前端怎么配，Claude CLI 都强制带 `--dangerously-skip-permissions`，跟 UI 不一致也加大权限风险。改为尊重前端值（false 时不加）；提醒：web 模式无终端，关闭后 CLI 可能因等待权限提示而 hang，是用户的选择。
- **NSIS 安装类型检测可被骗**：之前只看 `uninstall.exe` 是否存在，便携包目录里造一个空 `uninstall.exe` 就能让应用走"已安装版"自动更新流程拉错管线。改为同时校验 `HKCU` / `HKLM\…\Uninstall\<identifier>` 注册表项存在；新增 `winreg` 依赖（仅 Windows target）。

### Changed

- **`paths.rs` 模块新增**：把 session 文件路径校验从 `session-web/src/main.rs` 移到 `session-core/src/paths.rs`，Tauri 与 Web 共享一份实现，两边逻辑不再可能漂移。
- **`scripts/sync-version.mjs`**：把 `package-lock.json` 的两个 version 字段（顶层 + `packages[""]`）也加进 sync / check 范围，避免 lock 与 manifest 漂移阻塞构建。
- **`tsconfig.tsbuildinfo`** 加进 `.gitignore` 并 `git rm --cached`：构建产物不再进版本管理。

### Version

- 将工作区版本统一提升到 `2.12.0`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.11.0] - 2026-05-06

### Added

- **Codex 续聊改用 `codex app-server`（JSON-RPC over stdio）**：新增 `crates/session-core/src/codex_app_server.rs`，进程级单例按 `(api_key, base_url)` 指纹复用一条常驻 app-server，对外暴露 `start_thread / resume_thread / start_turn / interrupt_turn / subscribe`。`src-tauri/src/commands/chat.rs` 与 `crates/session-web/src/chat_ws.rs` 的 codex 分支不再每轮 spawn 一次 `codex exec`，新建会话走 `thread/start`，续聊走 `thread/resume`，取消走 `turn/interrupt`，Claude 路径完全不动。
- **续聊预载历史**：`thread/resume` 返回的 `thread.turns` 通过新增的 `replace_messages` action 注入 `chatStore`，前端进入 `/chat/:sessionId` 续聊后立即看到完整上下文 + 拼回用户的待发气泡，再衔接本轮回复。
- **codex `app-server` 通知解析**：`parseCodexStreamLine` 重写为消费 `thread/started` / `item/agentMessage/delta` / `item/completed`（agentMessage / reasoning / commandExecution / fileChange / webSearch / mcpToolCall）/ `turn/completed` / `turn/failed` / `error` / `thread/tokenUsage/updated` 等 app-server 事件；流式增量通过新增 `delta` action 折叠回同一条 assistant 气泡，token 用量在 `done` 时拼出"完成 [输入: x · 输出: y · 缓存命中: z]" 系统提示。

### Changed

- **codex 凭据解析支持嵌套 provider**：`cli_config.rs` 新增 `active_codex_provider()`，能识别 `~/.codex/config.toml` 里 `model_provider = "<name>"` + `[model_providers.<name>]` 这种新版嵌套配置（旧版顶层 `[provider]` 仍兼容），不再因 base_url 没读到而回落到 `https://api.openai.com`。
- **模型列表跟随 codex 配置**：`model_list::list_models` 的 codex 分支从 `cli_config::get_credentials("codex")` 取真实凭据（auth.json + config.toml + env），不再只看 env；同时新增 `join_models_endpoint()` 兼容 base_url 已含 `/v1` 的形态，不再追加成 `/v1/v1/models`。
- **聊天侧栏 ChatProcessState 扩展**：新增 `codex_turns` 与 `codex_sessions` 两个映射，cancel 路由能区分 Claude（杀进程）vs Codex（发 `turn/interrupt`）。

### Removed

- **删除"快速问答"功能**：移除 `src/components/quick-chat/`、`src/stores/quickChatStore.ts`、`startQuickChat`（tauri/web 两份）+ 相关 SSE 解析帮手、路由 `/quick-chat`、侧栏入口、`QuickChatMessage` 类型。后端同步删除 `crates/session-core/src/quick_chat.rs`、Tauri `quick_chat` 命令与注册、`session-web` 的 `quick_chat_handler` + `/api/quick-chat` 路由；`session-web/Cargo.toml` 顺手清理掉只服务于此的 `futures-util` / `tokio-stream` 依赖。

### Version

- 将工作区版本统一提升到 `2.11.0`，同步 `package.json`、`src-tauri/tauri.conf.json` 与 3 个 Cargo manifest。

---

## [2.10.0] - 2026-05-06

### Added

- **消息页用户体验打磨**（基于 commit `2f916a5` / `4bc739a`，本次 release 一并发布）：
  - `UserMessage` 折叠 / 复制按钮内联到气泡顶部，折叠按钮可一并收起该提问下的所有 Claude 回复，气泡和折叠预览处显示 "X 条回复" 徽标；复制范围补齐 `text + tool_result` 两类。
  - `MessageThread` 的 `ThreadBranch` 不再为 user 节点重复渲染折叠按钮，控制权下放给 `UserMessage`。`ToolOutputMessage` 每个工具输出块加复制按钮，复制原始未截断内容。
  - 顶栏 "全部展开/折叠" 状态持久化到 `localStorage`；新增 `useExpandAllControl` 的 `followGlobal` 选项，用户/助手主气泡跟随全局初始值。新增 6 色 `questionPalette` 循环，用户气泡 / TOC 侧栏 / Timeline dots 按提问序号共享同色。
  - 页头 "已加载 X / Y" 改为按钮：连续翻页直到加载完成，再次点击中止。`useReplyNotification` 在窗口失焦期间累积 `unreadCount`，回到页面后通过 banner + "跳到底部 / 关闭" 提醒，切换会话自动清零；右下角悬浮按钮组新增 expand/collapse 切换。
- **续聊体验改进**：
  - `ChatInput` 新增 `/rename <名字>` 斜杠命令，直接修改当前 session 的别名（空名清空），自动保留已有标签；执行成功后触发 `refreshInBackground(true)` 让侧栏列表立即反映新名称。底部提示行显示 `Ctrl+K 或 /model 切换 · /rename <名字> 改别名`，命令结果以胶囊形式向右滚出 2.5 秒。
  - `ChatHeader` 在工作目录右侧新增 sessionId 徽标（短 8 位 + Hash 图标），点击复制完整 ID，hover 提示对应的 `claude --resume <id>` / `codex resume <id>` 还原命令；流式中正常显示。
- **搜索匹配范围扩展**：搜索匹配范围切换条新增第 4 项「标签」，命中时结果以 `role: "tag"` 角色返回，前端展示为「标签」标签。后端 `SearchScope` 同步加 `Tags` 变种，`global_search` 在 Tags 模式下只比对 metadata 标签；All / Session / Content / Tags 四种匹配范围共享同一过滤器。
- **回收站 Web 模式支持**：`session-web` 新增 5 条 recyclebin 路由（`GET /api/recyclebin`、`POST /api/recyclebin/:id/restore`、`DELETE /api/recyclebin/:id`、`POST /api/recyclebin/empty`、`POST /api/recyclebin/cleanup-orphans`），与 Tauri 桌面端一致；`webApi.ts` 内原本抛 "Recyclebin is not available" 的 stub 全部替换为真实 fetch 调用。
- **分屏活动窗口高亮**：开启分屏后，活动 pane 通过 `border-primary + ring-primary/40` 描边突出显示；鼠标按下任一 pane 自动同步 `chatStore.activePaneId`，单分屏时不显示边框避免视觉噪音。
- **Thread 视图支持中间消息分叉**：`ThreadSummaryView` 改为基于 `parentUuid` 的真实树（用 `buildMessageTree` 构造），用户提问按父子层级缩进 + 虚线连接，节点带「回复上一条」、`N 条分叉` 等关系标签；每个节点新增「回复此处」按钮，调用现成的 `forkAndResume` 从该消息分叉新会话并在终端打开。
- **统计页区间消息数**：新增 `DailyTokenEntry.messageCount` 字段，Claude `FileStat.daily_messages` 按日累加 assistant 消息计数（cache version 1 → 2，首次打开统计页会重建一次缓存）；Codex `daily_map` 同步增加 messages 维度。前端 StatsPage 在选择时间范围（今天 / 本周 / 本月 / 30 天 / 自定义）时，"区间消息数"卡片随之刷新；选择"全部"时显示"总消息数（全期）"。

### Changed

- **滚动条默认更显眼**：`ScrollArea` 的 thumb 由 hover 才高亮改为默认带可见颜色（`muted-foreground/0.55`），hover 加宽 + 切到 primary 配色；track 默认 0.75rem，hover 扩到 0.875rem，单击 track 跳转、拖动 thumb 滚动均保持原行为。
- **回收站恢复链路**：`recyclebin::restore_item` 成功后根据 `item.source` 失效对应 sessions 缓存（claude / codex），解决"恢复后列表不刷新"的体感 bug；前端 `appStore.restoreItem` 同步触发 `refreshInBackground(true)`；新增跨卷场景的 copy + remove 回退（EXDEV / "different volume"）。

### Fixed

- **Web 模式 `node not found`**：续聊时 `#!/usr/bin/env node` 在 systemd / 容器等最小 PATH 环境下无法解析。`session-core/cli.rs` 新增 `find_node()`（which / nvm / npm-global / 系统路径多重 fallback），`chat_ws.rs` 与 `commands/chat.rs` 的 `compose_chat_path` 把 CLI 自身目录和 node 目录一并加进子进程 PATH。
- **Rust 1.95 clippy 告警**（commit `4d018f7`）：`model_list.rs` 的 `sort_by(cmp.reverse)` 改为 `sort_by_key(Reverse)`；`provider/codex.rs` `session_summary` 中 `function_call` / `function_call_output` / `reasoning` 三分支的内层 `if` 改写为 match guard，满足 `collapsible_match`。

### Version

- 将工作区版本统一提升到 `2.10.0`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、3 个 Cargo manifest 与 `Cargo.lock` 中的工作区包版本记录。

---

## [2.9.0] - 2026-04-23

### Added

- 左侧提问目录侧栏（`MessageTOCSidebar`）：在消息页主区左侧列出会话内所有用户提问（编号 + 两行预览 + 时间戳），点击跳转到对应消息并带 1.2s ring 高亮反馈。展开 240px、折叠 40px 细条，折叠状态持久化到 `localStorage.messageTocCollapsed`。
- Thread 摘要视图：顶栏新增按钮切换，将会话展平为"用户提问 → CC 回复摘要"卡片列表（含模型、时间、是否包含工具调用），点击任一条跳回消息视图并定位到该问答。
- 消息级整体折叠：`UserMessage` / `AssistantMessage` 接入 `useExpandAllControl`，顶栏"全部折叠"会把每条消息缩为单行预览，每条消息旁也有独立的展开/折叠按钮。
- 思考过程块默认折叠、每个独立可展开；每条回答的结构化显示更接近"思考过程（折叠）+ 正式回答"。
- 选中消息文本浮动 Reply 按钮（`SelectionReplyButton`）：在消息滚动区内划选任意文本即弹出胶囊按钮，点击把选中内容按行包成 `>` markdown 引用块注入续聊输入框并聚焦；Thread 模式或流式中自动禁用。
- 首问粘性横幅：消息滚动区顶部常驻显示当前已加载消息的首条提问摘要，滚动不消失；点击跳回顶部，当首问本身已进入视口时自动淡出避免重复。
- 文本复制公共工具 `copyTextToClipboard`（`await navigator.clipboard` + `document.execCommand` 回退），`UserMessage` / `AssistantMessage` / 工具结果卡片 `CopyButton` 共用。

### Changed

- `ExpandAllContext.useExpandAllControl` 首次挂载不再用全局 `ctx.expanded` 覆盖块自身的 `defaultExpanded`，只在版本号变化（用户主动点击全局按钮）时同步；思考块等默认折叠块现在真正以折叠态登场。
- `ChatInput` 改为 `forwardRef`，暴露 `insertQuote` / `focus` 句柄；在 `appStore` 新增 `reloadLatestMessages()`（无视 `messagesPage` 守卫强刷最新一页）。续聊流式完成后延时触发后台刷新 + `reloadLatestMessages` + 清空续聊 pane，让历史消息列表接管显示，解决需手动刷新才能看到新对话。
- `MessageThread.handleDotClick`（被 TOC、时间轴、首问横幅共用）加防御：未找到目标退化到回滚视口顶部；找到后加 `ring-2 ring-primary/40` 1.2s 高亮反馈。
- 移除原有悬浮提问跳转球 `UserQuestionJumpList` 在消息页的使用（组件文件保留，避免破坏其他引用）。

### Fixed

- 修复复制按钮点击无响应：此前三处 `navigator.clipboard.writeText(...)` 均未 await 且无回退，非 HTTPS / 权限不足场景下会静默失败却仍显示"已复制"。
- 修复进入会话后"一直在重复刷新"：`MessagesPage` 的"视口不足就自动拉更早消息"副作用限制为每次进入会话最多触发一次，避免思考块默认折叠后内容变短引起的多页连环加载。
- 修复 TOC 点击无响应：先前作为 `absolute + pointer-events-none` 外层包裹时，`pointer-events-auto` 在部分嵌套下并未恢复，导致 TOC 条目点击被吞。
- 修复 TOC 随右侧/全局滚动一起被带走：`MessagesPage` 根由 `h-full` 改为 `h-dvh max-h-dvh` + `overflow-hidden`，切断与 `scroll-area-content` 的百分比循环依赖，整个消息页被钉在视口高度内；左侧提问目录和右侧消息各自独立滚动，外层 `ScrollArea` 不再能卷走整页。

### Version

- 将工作区版本统一提升到 `2.9.0`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、3 个 Cargo manifest 与 `Cargo.lock` 中的工作区包版本记录。

## [2.8.2] - 2026-04-08

### Fixed

- 修复长会话在消息页首屏仅显示最近 100 条后缺少可靠后续入口的问题；主视图与分屏视图都补充了显式“加载更早的消息”按钮，并在首屏内容不足以形成滚动区域时自动继续补页，不再容易误判为只能看 100 条消息。
- 修正消息分页继续加载时的状态回写逻辑，避免异步翻页结果在会话切换后串入当前视图。

### Changed

- 将工作区版本统一提升到 `2.8.2`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、3 个 Cargo manifest 与 `Cargo.lock` 中的工作区包版本记录。

### Documentation

- 更新 README 顶部 `Latest in v2.8.2` 摘要与消息分页说明，补充“顶部加载按钮 + 首屏自动补页兜底”的行为描述。

## [2.8.1] - 2026-03-29

### Fixed

- 修复桌面端打包版本与 `tauri dev` 开发模式下启动即白屏的问题；根因是前端 API 入口使用顶层 `await`，在部分 WebView 运行时会在模块初始化阶段直接中断。

### Changed

- `src/services/api.ts` 改为同步导出的懒加载代理，按需动态导入 `tauriApi` / `webApi`，避免基础服务层在应用启动时依赖顶层 `await` 语法支持。
- 将工作区版本统一提升到 `2.8.1`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、3 个 Cargo manifest 与 `Cargo.lock` 中的工作区包版本记录。

### Documentation

- 更新 README 顶部 `Latest in v2.8.1` 摘要，补充本次白屏修复与 API 启动链路调整说明。

---

## [2.8.0] - 2026-03-29

### Added

- 消息页支持多会话分屏浏览与继续对话，可在左右分屏和上下分屏之间切换，每个分屏都能独立查看和续聊。
- 新增线程视图：根据 `parentUuid` 与 `@mention` 解析消息分叉关系，支持折叠分支、语义标题展示、从历史节点继续追问，以及当前会话提问定位悬浮球。
- `AskUserQuestion` 工具结果支持在界面内直接填写答案并提交，页面失焦时补充网页标题与系统通知提醒。

### Changed

- 全局搜索新增 `all / session / content` 范围过滤、会话分组视图、会话名称/别名匹配与标签筛选联动，搜索结果与会话页之间的跳转链路更完整。
- Chat 页面优先由 URL 会话 ID 驱动，修复新建会话和继续会话之间的状态串扰，并让 Codex 默认模型接入自动选型链路。
- Web 前端改为按页面懒加载，并补充 vendor 分包策略，降低首屏主 bundle 压力。
- 将工作区版本统一提升到 `2.8.0`，同步 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、3 个 Cargo manifest 与 `Cargo.lock` 中的工作区包版本记录。

### Fixed

- 修复 Web 模式文件监听后 Claude / Codex 列表缓存未及时失效的问题，避免刷新后仍读取旧的 session 数据。
- 恢复空会话/无效会话的清理链路，并统一搜索候选提取与提前停止逻辑，减少无效扫描和结果偏差。
- 修复 Tauri CLI 对话在新会话与 `/chat/:sessionId` 续聊场景下的事件绑定时序问题，避免漏收 `chat-output`、`chat-error`、`chat-complete` 事件。
- 修复浏览器环境下回复通知兼容性问题，避免 `NotificationOptions` 差异导致提醒失效。

### Performance

- 消息加载改为优先走尾部分页与基于文件 `mtime` 的缓存，重复打开同一会话时不再为最后 50 条消息完整物化整份消息向量。
- 收敛聊天页与消息页的重复派生计算，减少 `toolResultMap`、turn 切分、线程建树和滚动按钮状态带来的无效重算。
- 对消息叶子组件、流式消息链路和长历史线程视图补充 memo 与渐进挂载，降低首次打开长会话时的主线程压力。
- 超大 Markdown、超长代码块以及工具视图中的大 JSON / 大源码默认先走纯文本轻量展示，按需再启用完整 Markdown 渲染或语法高亮。

---

## [2.7.5] - 2026-03-26

### Added

- 新增侧边栏「无效项管理」入口和 `/cleanup` 页面，按项目集中展示路径失效项目与空会话。

### Changed

- cleanup 页面支持批量选择后统一清理；桌面端会话删除进入回收站，Web 端删除为永久删除，`codex` 数据源暂不支持删除无效项目索引。
- 同步工作区发布版本到 `2.7.5`，覆盖 `package.json`、`src-tauri/tauri.conf.json`、3 个 Cargo manifest，以及 `Cargo.lock` 中的 workspace 包版本记录。

### Documentation

- 更新 README 顶部 `Latest in v2.7.5` 摘要，并补充「无效项管理」功能说明。
- 新增本节记录 `2.7.5` 发布内容与版本同步范围。

---

## [2.7.4] - 2026-03-26

### Changed

- Corrected the project release version to `2.7.4` in `package.json`.
- Synced the release version metadata used by the workspace versioning workflow.

### Documentation

- Updated the README release summary to reflect `v2.7.4`.
- Added this changelog entry to document the `2.7.4` release.

---

## [2.7.3] - 2026-03-26

### Fixed

- Project-list caching now takes effect: `get_projects()` no longer always calls `refresh_projects_cache()` before checking cached entries.
- Claude list-cache reads are reused in-process so a single request does not repeatedly read and deserialize `claude-list-cache.json`.
- Uncached project `sessionCount` now falls back to valid session files instead of raw `.jsonl` file counts, reducing first-load count drift.
- `delete_project()` now reads `sessions-index.json` once and reuses the result for both the real project path and recycle-bin display name.
- `save_cache()` now writes via `tmp + rename` to reduce the risk of truncated cache files during interrupted writes.

### Documentation

- README now includes a `v2.7.3` summary for the cache, count-consistency, and delete-flow changes.

---

## [2.7.2] - 2026-03-26

### Changed

- **项目列表加载策略重构**：Claude 和 Codex 启动时改为快速扫描工程目录，项目列表优先显示“上次进入工程后精确扫描得到的缓存会话数”，没有缓存时才回退到快速计数；不再在启动阶段对所有工程执行全盘精确扫描
- **工程详情按需精确扫描**：仅在用户点进某个工程时，才对该工程执行精确会话扫描，并将结果持久化到本地缓存，供下次启动直接复用
- **后台静默刷新**：桌面端对当前数据源新增 10 分钟一次的后台缓存刷新；文件系统监听到相关变更时也会失效缓存并强制刷新，尽量保证列表数据“秒开但不过时”

### Fixed

- **修复 Claude/Codex 源切换后会话页卡在”加载中”**：切换数据源时现在会同步重置 loading 状态，并为项目/会话/消息请求增加 source 级 stale check，旧请求返回后不再把新页面状态卡死
- **启动更新提示改为确认式交互**：桌面端启动后约 1.5 秒自动检查更新，检测到新版本时直接弹出确认框；安装版可立即更新并重启，便携版可直接跳转下载页
- **文件写入改为原子操作**：`project meta`、`sessions-index.json`（Fork 后 / Terminal 录制写回）三处直接写入均改为 tmp + rename 模式，进程在写入中途被中断时不会产生截断或损坏的 JSON 文件

### Documentation

- 更新 README 中关于项目扫描、会话加载、后台刷新和应用内更新的行为说明

---

## [2.7.1] - 2026-03-25

### Performance

- **会话列表加载大幅提速**：将 `get_sessions` 内部的 5 次独立文件读取（`extract_first_prompt` / `extract_custom_title` / `extract_session_metadata` / `count_messages` / `count_messages_result`）合并为单次 pass（`scan_session_file_once`），287 个 session 的项目加载耗时约减少 80%

### Fixed

- **禁止读取操作触发文件系统写操作**：`get_sessions` 不再自动将无效 session 移入回收站；清理操作必须由用户手动触发，符合最小惊讶原则，消除潜在数据丢失风险
- **修复正在运行的 Claude Code 会话被误删**：`cleanup_all_orphan_dirs` 现在跳过最近 5 分钟内修改的目录，避免 CC 刚创建但尚未写入消息的 session 文件或 subagent 目录被误判为无效
- **修复 Corrupt 误判**：进程被强制终止（SIGKILL）后，JSONL 文件最后一行通常为截断的 JSON；现在只有非末行解析失败才判定为 Corrupt，末行截断静默忽略
- **修复侧边栏 session 计数偏大**：`count_jsonl_files`（用于项目列表页的 session 数量显示）改为快速字符串扫描确认文件含有至少一条 user/assistant 消息行，与 `get_sessions` 的过滤逻辑保持一致

---

## [2.7.0] - 2026-03-25

### Added

- **回收站**：删除会话/清理无效会话/清理空工程目录时，改为移入软删除回收站（`~/.claude-code-viewer/recyclebin/`），可从侧边栏"回收站"页面查看、恢复或永久删除，支持一键清空回收站
- **Codex CLI 支持**：在对话、设置页面新增对 Codex CLI 的完整支持，包含 API Key / Base URL 配置、模型列表、CLI 路径检测

### Changed

- **Windows 路径解码改进**：`path_encoder` 新增 `resolve_segments_partial` 逐级模糊匹配，无法完整解析时返回有意义的部分路径而非原始编码字符串；修复 `X--Users-...` 双横线前缀解析逻辑

### Fixed

- **Homebrew nvm 检测**：登录 Shell 回退新增显式 source `~/.zshrc` / `~/.bashrc`，解决 Homebrew 安装的 nvm 因初始化写在 `.zshrc` 而非 `.zprofile` 导致 Codex/Claude 二进制检测失败的问题；新增 `/opt/homebrew/var/nvm` 和 `/usr/local/var/nvm` 作为备用 `NVM_DIR` 扫描路径（macOS）

---

## [2.6.4] - 2026-03-24

### Added

- **CLI 安装方式选择器**：对话设置中新增安装方式 chip（npm / nvm / bun / 手动），点击「重新检测」失败后，根据所选方式展示对应的预期路径和操作提示，帮助用户快速定位 CLI 可执行文件；所选方式持久化到 localStorage

### Changed

- **使用统计改为直接扫描 JSONL**：不再依赖 `~/.claude/stats-cache.json`（由 Claude Code 定期更新，可能滞后数周），改为直接扫描 `~/.claude/projects/**/*.jsonl`，token 统计始终反映最新会话数据
- **统计索引本地缓存**：以 per-file mtime 为 key 缓存每个 JSONL 文件的统计结果，保存于系统 config 目录（`~/.config/ai-session-viewer/`，Windows: `%APPDATA%\ai-session-viewer\`），不污染 `~/.claude/`；下次加载只重新扫描有变动的文件
- **并行扫描加速**：使用 rayon 并行处理需要重新扫描的文件，并在 JSON 解析前通过字符串预过滤跳过无 `usage` 字段的行
- **首次建立索引提示**：首次打开统计页时，若本地无缓存，界面提示「正在建立统计索引，会话较多时可能需要较长时间」，有缓存时仅显示普通加载状态

### Fixed

- **CLI 自动检测**：应用启动时若未检测到 CLI，自动触发一次检测，无需手动进入设置点击按钮
- **nvm 路径扫描**：Unix/Mac 新增 `zsh/bash -l -c "which claude"` login shell 回退，解决 Tauri 进程不继承 nvm PATH 导致检测失败的问题；Windows 补充扫描 nvm-windows 路径（`%APPDATA%\nvm\{version}\claude.cmd`）；同时支持 `$NVM_DIR` 自定义 nvm 根目录

---

## [2.6.3] - 2026-03-24

### Added

- **自动更新通知 Toast**：检测到新版本时右下角自动弹出浮层提醒，显示版本号变化（`v旧 → v新`）；安装版一键"更新并重启"，便携版直接跳转 GitHub Release 下载页；可忽略当前版本，状态持久化到 localStorage

### Changed

- **删除工程对话框简化**：`DeleteProjectDialog` 重写为 checkbox 式单一 UI，移除输入项目名强确认和 Git 状态检查；新增"同时清理 `~/.claude.json` 项目配置"可选项；移除源代码目录删除功能（降低误操作风险）
- **会话管道改为磁盘优先扫描**：`get_sessions()` 改为三步流水线（扫描磁盘有效/无效 jsonl → 从 index 补全元数据 → 将无效文件移入 `invalid/`），会话列表更准确；`get_projects()` 恢复轻量 `count_jsonl_files()`，避免列表页全量扫描文件内容
- **删除级别枚举化**：新增 `DeleteLevel`（`SessionOnly` / `WithCcConfig`）和 `DeleteResult`，`delete_project()` 支持按级别清理 `~/.claude.json` 项目条目及书签；后端增加路径穿越防护

### Fixed

- **Markdown 表格渲染**：修复 `isAsciiArtLine()` 将表格分隔行（`|---|---|`、`| --- | :---: |`）误判为 ASCII 图表并包入代码块的问题，表格现可正确渲染

---

## [2.6.2] - 2026-03-23

### Added

- **删除工程同时删除源代码**：工程操作菜单新增"删除会话数据和源代码"选项，支持同时删除 Claude 会话数据和本地源代码目录；删除前检查 Git 状态（未提交更改、未推送提交），要求输入项目名确认，防止误操作
- **源代码状态检查 API**：新增 `check_project_source_status` 命令，检测项目源代码目录是否存在、是否为 Git 仓库、是否有未提交更改或未推送提交
- **共享删除确认对话框**：抽取 `DeleteProjectDialog` 组件，统一项目列表页和侧边栏的删除确认交互，支持简单确认和强确认（输入项目名）两种模式
- **危险路径保护**：删除源代码时自动拒绝根目录、Home 目录、系统目录等危险路径

### Fixed

- **Session 计数虚高**：`get_projects()` 和 `get_sessions()` 现在验证 `sessions-index.json` 中条目对应的 .jsonl 文件是否真实存在，过滤已被 Claude Code 清理的"幽灵条目"，侧边栏会话计数不再虚高

---

## [2.6.1] - 2026-03-23

### Fixed

- **项目路径智能解码**：新增 `decode_project_path_validated` 文件系统验证解码器，当 `sessions-index.json` 缺少 `originalPath` 时，通过逐段匹配文件系统目录项恢复原始路径（含中文、`.` 等被编码为 `-` 的字符），解决有损解码导致路径显示错误的问题
- **路径存在性检测**：`ProjectEntry` 新增 `pathExists` 字段，后端在解码路径后自动检测路径是否存在于文件系统
- **不存在路径视觉提示**：侧边栏和项目列表页对路径不存在的项目显示黄色警告图标和 tooltip 提示，帮助用户识别已删除或移动的项目目录
- **Unix 隐藏目录启发式**：支持 `--` → `.` 的启发式匹配（如 `.claude`、`.config` 等隐藏目录）
- **项目/会话切换竞态条件**：`selectProject` 和 `selectSession` 异步返回后增加 stale check，切换时立刻清空旧 sessions，避免闪显上一个项目的会话列表
- **sessions-index.json 解析容错**：`fileMtime` 字段兼容 f64 浮点数（截断为 u64）；解析失败时自动 fallback 到逐文件扫描，不再直接报错

---

## [2.6.0] - 2026-03-22

### Added

#### CLI 对话增强
- **自定义 CLI 路径**：聊天设置中新增 CLI 路径配置项，支持手动指定 Claude CLI 可执行文件路径，持久化到 localStorage
- **流式增量输出**：启用 `--include-partial-messages` 标志，解析 `stream_event` 的 delta/block_start 实现实时文本显示
- **工具查看器暗色适配**：FieldList 背景色改用 theme-aware 的 `bg-muted`，深色主题下不再刺眼

#### ASCII 图表渲染优化
- **自动检测 ASCII 图表**：新增 `wrapAsciiArt()` 预处理函数，在 Markdown 渲染前自动识别包含 box-drawing 字符、管道符/破折号布局的文本行，将连续的 ASCII 图表行包裹进代码块
- **代码块样式修复**：`pre` 组件从 `<div>` 改回真正的 `<pre>` 标签，保留空白符格式 + `overflow-x-auto` 水平滚动；通过 Tailwind 任意变体清除内部 `code` 的内联样式冲突
- 覆盖 AssistantMessage、StreamingMessage、MarkdownContent 三处 Markdown 渲染路径

### Fixed

- **Windows CLI 发现**：过滤非可执行文件，仅保留 `.cmd`/`.exe`；`known_paths` 新增 `.cmd` 路径覆盖 npm 安装场景
- **项目路径回退**：当 `sessions-index.json` 缺少 `originalPath` 时，从首条 entry 的 `projectPath` 回退，避免路径显示为编码字符串
- **macOS CI 构建**：合并 ARM 和 Intel 两个构建为 `universal-apple-darwin` 单一构建，一个 DMG 同时支持 Intel 和 Apple Silicon，修复 x86_64 跨编译 DMG 打包失败的问题
- **Clippy too_many_arguments**：为 `continue_chat` 命令添加 `#[allow(clippy::too_many_arguments)]`

---

## [2.5.0] - 2026-03-22

### Added

#### Session 命名与 Claude Code `/rename` 双向同步
- **读取同步**：加载 Claude session 时，自动从 JSONL 文件扫描 `custom-title` 记录，将 Claude Code `/rename` 设置的名称作为 session alias 显示；多次重命名取最后一条
- **写入同步**：在 app 内编辑 session alias 时，直接向 JSONL 追加 `{"type":"custom-title","customTitle":"..."}` 记录，与 Claude Code `/rename` 格式完全一致，JSONL 成为 Claude session 名称的唯一真实来源
- **清空支持**：清空 alias 时追加空 `customTitle` 记录，下次读取自动回退到首条用户消息（`firstPrompt`）
- **Codex 不受影响**：Codex session 的 alias 依然走 `.session-viewer-meta.json`，逻辑不变

#### 后端实现
- `session-core/parser/jsonl.rs` 新增 `extract_custom_title(path)`：扫描 JSONL 返回最后一条非空 customTitle（空字符串视为清空意图返回 None）
- `session-core/parser/jsonl.rs` 新增 `append_custom_title(path, session_id, title)`：向 JSONL 追加 custom-title 记录
- `provider/claude.rs` 的 `scan_single_session` 和 index 加载路径均调用 `extract_custom_title` 填充 alias
- Tauri command `update_session_meta` 和 Axum handler 新增 `file_path` 参数：Claude source 写 JSONL，Codex source 继续写 metadata

#### 前端
- `appStore.updateSessionMeta` action 内部查找当前 session 的 `filePath` 并传给 API
- `tauriApi.ts` / `webApi.ts` 的 `updateSessionMeta` 函数新增 `filePath` 参数

---

## [2.4.0] - 2026-03-22

### Added

#### 工程操作菜单（⋯ 按钮）
- **统一操作入口**：用 `⋯`（MoreHorizontal）按钮替换原有的 hover 删除图标和右键上下文菜单，鼠标悬停时在项目卡片右上角和侧边栏项目行右侧显示，点击展开 portal dropdown 菜单
- **路径复制**：dropdown 顶部显示工程完整路径，右侧复制图标一键写入剪贴板，成功后 300ms 切换为绿色对勾；剪贴板 API 不可用时静默降级
- **工程别名**（仅 Claude 数据源）：可为工程设置自定义显示名称（不修改磁盘目录），别名持久化存储在各工程目录下的 `.project-meta.json`；已设置别名时卡片标题显示别名，原始目录名以小字辅助显示
- **删除工程**（仅 Claude 数据源）：从 dropdown 进入删除确认对话框，行为与之前一致
- **Codex 数据源**：dropdown 仅显示路径复制，不显示别名/删除入口（与现有 Codex 不可删除的行为一致）

#### 后端实现
- `session-core` 新增 `get_project_alias` / `set_project_alias`，别名存储在 `.project-meta.json`（使用 `serde_json::Map` 原始操作以保留未来扩展字段）；两个函数均包含 `canonicalize + starts_with` 路径遍历防护
- `ProjectEntry` 新增 `alias: Option<String>` 字段，`list_projects` 时自动读取并合并
- Tauri command `set_project_alias` + Axum `PUT /api/projects/alias` 路由双端均已实现

#### 前端
- `ProjectActionsMenu`：新建 portal-based dropdown 共享组件，`ProjectsPage` 和 `Sidebar` 共用
- `appStore.setProjectAlias`：乐观更新（立即更新本地 projects 列表）+ 失败回滚

---

## [2.3.0] - 2026-03-22

### Added

#### 工程删除
- **Hover 删除图标**：鼠标悬停到项目卡片时，右上角显示 `Trash2` 删除图标（仅 Claude 数据源可见），点击弹出确认对话框
- **右键菜单**：项目卡片支持右键呼出上下文菜单，显示工程信息（名称、路径、会话数）及删除入口（仅 Claude 数据源），菜单自动避开视口边缘
- **删除确认对话框**：展示工程名、路径、会话数量，操作不可逆，需二次确认；删除完成后自动导航回项目列表并清空下游状态
- **后端实现**：`session-core` 新增 `delete_project` 函数，调用 `std::fs::remove_dir_all` 递归删除目录（含 `.session-viewer-meta.json`）；同时包含路径遍历防护（`canonicalize + starts_with` 验证）；Tauri command + Axum `DELETE /api/projects` 路由双端均已实现，缺失项目返回 HTTP 404

#### 空会话批量清理
- **清理按钮**：SessionsPage 标题行，当项目内存在 `messageCount === 0` 的空会话时，显示「清理空会话 (N)」按钮
- **清理对话框**：列出所有空会话（SessionID 前缀 + 最后修改日期），支持逐条勾选或全选/取消全选，确认后批量删除并刷新会话列表

### Fixed

#### 流式回复内容重复
- **Web 模式**：`useChatStream` 中 `getChatWebSocket()` 返回模块级单例，断线重建后新旧对象不同，`removeEventListener` 对新对象无效导致监听器泄漏叠加；修复：保存 ws 快照到局部变量，`handleMessage` 定义在 `setupWebSocket` 内部与快照绑定，cleanup 对同一快照调用 `removeEventListener`
- **Tauri 模式**：`setupListeners` 在 `await import(...)` 完成后未检查 `cancelled`，组件卸载后监听器仍可能注册；修复：在 `await import` 之后、`await listen` 之前加 `if (cancelled) return` 检查，并在 cleanup 函数中直接设置 `cancelled = true`

#### 消息不自动刷新
- `refreshInBackground` 仅刷新 projects + sessions，Claude Code 写入 JSONL 后消息区不更新；在函数末尾追加静默消息刷新：仅当 `messagesPage === 0`（用户未上翻历史）时调用 `api.getMessages`，更新 `messages / messagesTotal / messagesHasMore`，不触发 loading 遮罩，失败静默忽略

#### 搜索进入会话无法对话
- 从全局搜索跳转到某个会话时，`appStore.sessions` 为空（用户未先进入项目页），`sessions.find(filePath)` 返回 `undefined`，导致 ChatInput 隐藏；修复：`MessagesPage` 的 `filePath` useEffect 改为异步，当目标 `filePath` 不在当前 sessions 中时先 `await selectProject(projectId)` 加载正确项目，再调用 `selectSession`，全程通过 `cancelled` 标志防止竞态

---

## [2.2.1] - 2026-03-16

### Fixed

#### Web 服务器默认绑定地址
- **默认监听所有网卡**：`session-web` 默认绑定地址从 `127.0.0.1` 改为 `0.0.0.0`，直接用本机 IP 或局域网地址访问无需再手动指定 `--host`
- **旧行为**：仅接受 localhost 连接，非 localhost IP 访问时浏览器无响应
- **新行为**：监听所有网卡，可通过任意 IP 直接访问；如需限制来源仍可用 `--host <ip>` 或 `ASV_HOST=<ip>` 覆盖

#### Web 前端 crypto.randomUUID 兼容性
- **HTTP 非 localhost 场景**：通过 HTTP（非 HTTPS）且非 localhost 访问时，浏览器限制 `crypto.randomUUID()` 仅在安全上下文可用，导致发送消息时报错
- **polyfill 降级**：新增 `generateUUID()` 函数，安全上下文使用原生 `crypto.randomUUID()`，否则自动降级为基于 `crypto.getRandomValues()` 的 RFC 4122 兼容实现（`getRandomValues` 在非安全上下文仍可用）
- **覆盖范围**：`parseClaudeStreamLine`（assistant / user / result 消息）、`startNewChat`、`continueExistingChat` 共 5 处调用全部替换

### Security

> **⚠️ 重要提示**：`0.0.0.0` 意味着服务器会在**所有网络接口**上监听，包括公网网卡。请务必根据部署环境配置以下安全措施：
>
> - **内网自用**：防火墙限制端口仅允许可信 IP，或使用 `--host <局域网IP>` 绑定到指定网卡
> - **公网暴露**：必须设置 `--token` / `ASV_TOKEN` 启用 Bearer Token 认证
> - **生产环境**：前置 Nginx / Caddy 反向代理，启用 HTTPS / TLS，避免明文传输

---

## [2.2.0] - 2026-03-16

### Added

#### Tool Output 折叠与 Markdown 渲染
- **折叠支持**：`>_ Tool Output` 区域内容超过 400 字符时默认折叠，标题栏显示字符数，点击可展开；短内容默认展开，用户可手动折叠
- **MD / 源码切换**：标题栏常驻 `</>` / `MD` 切换按钮，随时在 Markdown 渲染和原始文本之间切换；MD 模式使用 ReactMarkdown + remark-gfm，支持标题、表格、代码块等格式
- **两种 block 统一**：`tool_result` 和 `function_call_output` 两种类型统一使用 `OutputBlock` 子组件，行为完全一致
- **错误状态保留**：`tool_result` isError 时保持红色边框和背景

### Fixed

#### ToolViewer 长行自动换行
- **代码视图**：ToolViewer 切换到 `</>` 代码模式时，JSON 中超长字符串（如 `content` 字段大段文本）现在按容器宽度自动换行，不再出现水平滚动条
- **Read / Write 内容视图**：ReadContent 和 WriteContent 的语法高亮区域同步启用 `wrapLongLines`，读取或写入的长行代码不再水平溢出

### Refactored

- **MarkdownContent 提取为共享组件**：将 `ToolViewers.tsx` 内部的 Markdown 渲染函数提取为 `src/components/message/MarkdownContent.tsx` 独立导出组件，`ToolOutputMessage` 可直接复用，消除重复逻辑

---

## [2.1.0] - 2026-03-15

### Added

#### ToolViewer 代码/预览切换
- **Code/Preview 切换按钮**：ToolViewer 顶栏新增代码/预览双模式切换按钮（Code2 图标），支持在结构化字段视图与原始代码视图之间切换，方便调试时查看完整原始内容

#### 统计页时间范围筛选
- **时间范围过滤器**：统计页新增时间范围下拉筛选（今日 / 本周 / 本月 / 近 3 月 / 近半年 / 今年 / 自定义），精确统计指定时段内的 token 用量和活跃度
- **数据范围标签**：统计摘要卡片新增当前筛选范围说明文字，清晰标注数据来源周期

### Fixed

#### 统计页日期逻辑
- **周预设 Sunday bug**：修复"本周"预设将周日计算为下周起点的问题，改为以周一为一周开始
- **自定义日期重置**：切换预设时正确清空自定义日期输入框，避免残留值干扰筛选

#### ToolViewer 显示优化
- **Bash 工具默认展开**：Bash 工具调用默认展开显示，无需手动点击即可看到命令内容
- **完整命令显示**：修复长命令被截断的问题，现在显示完整命令文本
- **浅色模式文字**：修复 ToolViewer 在浅色主题下文字颜色不可读的问题
- **Code2 按钮折叠状态标题**：优化折叠状态下切换按钮的 title 提示文字，准确反映当前可切换的目标模式

#### 代理兼容
- **Bearer Auth 支持**：快速问答和 CLI 对话检测 API Key 时新增对 `ANTHROPIC_AUTH_TOKEN` 环境变量的读取，使用自定义代理（`ANTHROPIC_BASE_URL`）时发送 `Authorization: Bearer` 认证头，与主流代理服务兼容

---

## [2.0.1] - 2026-03-15

### Fixed

#### 工具调用展开渲染优化
- **统一 tool_use 渲染**：`AssistantMessage` 中的工具调用块改为复用 `ToolViewer`（原先为独立的裸 JSON `<pre>` 显示），现与会话详情页行为一致，支持结构化字段展示和 markdown 渲染
- **Markdown 文件内容渲染**：`Read` / `Write` 工具展开时，若目标文件为 `.md` / `.mdx`，内容改用 `ReactMarkdown + remark-gfm` 渲染（支持表格、代码块、链接），不再显示原始 `\n` 转义字面量
- **DefaultContent 结构化显示**：未识别工具不再裸输出原始 JSON；改为解析后按字段名对齐显示（中文标签 + 固定宽度列），并自动过滤 `timeout`、`run_in_background`、`dangerouslyDisableSandbox` 等噪声字段；短值内联，长值 / 代码值使用深色代码块

#### Linux 环境变量读取
- **Shell rc 文件 fallback**：在 Linux 上，桌面应用（Tauri）和 Web 服务器二进制（session-web）通过 systemd service 或桌面快捷方式启动时，不会自动 source `.bashrc`，导致用户在 shell 中配置的 `ANTHROPIC_API_KEY` 等变量无法读取。新增静态解析 `~/.bashrc`、`~/.bash_profile`、`~/.profile`、`~/.zshrc`、`~/.zprofile` 作为最低优先级 fallback，仅在 `#[cfg(unix)]` 下生效，Windows 无影响

---

## [2.0.0] - 2026-03-15

### Added

#### 搜索增强
- **搜索会话名称**：全局搜索现在同时匹配会话的自定义别名（alias）和首条 Prompt，搜索结果中标记为"会话名"角色，消息模式和会话模式均可展示
- **总消息数显示**：搜索会话模式中，每个会话卡片新增"X 条匹配 / 共 N 条"统计，方便快速判断会话规模
- **复制会话名**：会话卡片标题悬停时右侧出现复制图标，可直接复制别名或首条 Prompt，不触发会话跳转
- **跳转匹配消息**：从搜索结果点击进入会话时，自动滚动定位到第一条匹配消息并高亮（复用现有 `scrollTo` 机制）

#### 消息详情
- **加载进度指示器**：消息详情页顶部新增"已加载 N / M 条消息"文字计数和细进度条，解决分页加载时滚动条位置与实际内容长度不对应的问题；全部加载完成后自动隐藏进度条

### Fixed

#### 显示优化
- **ANSI 转义码清除**：工具调用结果（tool_result）、函数调用输出（function_call_output）、工具调用参数及 AI 文本块中的 ANSI 转义码（颜色码、光标控制、OSC 标题序列、BEL 字符等）现在被正确过滤，终端输出不再显示为 `\x1b[32m` 等乱码
- **ANTHROPIC_AUTH_TOKEN 环境变量未识别**：快速问答 / CLI 对话的 API Key 检测新增对 `ANTHROPIC_AUTH_TOKEN` 环境变量的读取（之前仅检测 `ANTHROPIC_API_KEY`），与 Claude CLI 的环境变量命名保持一致

---

## [1.9.7] - 2026-03-04

### Fixed

#### Web 模式 CLI 对话稳定性修复
- **CLI 启动错误无前端反馈**：当 CLI 未安装或启动失败时，错误仅记录在服务端日志，WebSocket 不发送任何消息，前端永远停留在"等待响应"状态。修复后通过 WebSocket 发送 error + complete 消息，前端正确显示错误信息（如"Claude CLI not found"）
- **CLI 权限确认导致卡死**：Web 模式下 CLI 子进程继承了 session-web 的 stdin，当 CLI 需要交互式权限确认时无限等待，因为 Web 前端无法提供终端输入。修复：将 stdin 设为 `Stdio::null()`，并在 Web 模式下自动传递 `--dangerously-skip-permissions` 参数

### Improved

#### 开发工具链
- `.gitignore` 新增 `.playwright-mcp/` 忽略规则

---

## [1.9.6] - 2026-03-04

### Improved

#### Web 服务器 musl 静态编译 — 彻底解决跨发行版兼容性
- **CI**：web-server 构建目标从 glibc 动态链接改为 `x86_64-unknown-linux-musl` 静态链接，产出零系统依赖的独立二进制
- **效果**：不再依赖特定 GLIBC 版本，Rocky 8/9、Ubuntu 18+、Debian、Alpine、CentOS 等任何 Linux 发行版下载即可运行
- **Docker**：编译阶段从 `rust:1-bookworm` 改为 `rust:1-alpine`（原生 musl 环境），运行时阶段从 `debian:bookworm-slim`（~80MB）改为 `alpine:latest`（~5MB），镜像体积大幅缩减
- **根因回顾**：v1.9.2～v1.9.4 反复修复的 GLIBC 不兼容问题（GitHub Actions runner GLIBC 2.39 vs 运行环境 GLIBC 2.34/2.36），通过 musl 静态编译从根本上消除

#### 更新面板体验优化
- **Release Notes Markdown 渲染**：更新面板中的版本说明从纯文本改为 Markdown 渲染（ReactMarkdown），标题、加粗、代码块等格式正确显示
- **动态 Release Notes**：CI 发布时从 CHANGELOG.md 自动提取当前版本的更新内容作为 GitHub Release body，用户检查更新时直接看到具体改了什么，不再是"See CHANGELOG"链接
- **显示区域扩大**：Release Notes 区域最大高度从 80px 提升到 192px，避免内容被截断

---

## [1.9.4] - 2026-03-03

### Fixed

#### CI Docker 镜像 GLIBC 不兼容
- **根因**：v1.9.2 的 CI 优化将 Docker 镜像构建改为复用 `web-server` job 在 GitHub Actions runner（ubuntu-22.04）上编译的二进制，但 runner 的 GLIBC 已被滚动更新到 2.39，而运行镜像 `debian:bookworm-slim` 只有 GLIBC 2.36，导致容器启动即崩溃（`GLIBC_2.39 not found`）
- **修复**：CI 的 docker job 改回使用仓库的多阶段 Dockerfile 构建（`rust:1-bookworm` 编译 + `debian:bookworm-slim` 运行），确保编译和运行环境 GLIBC 版本一致，移除不再需要的 artifact 传递

---

## [1.9.3] - 2026-03-03

### Improved

#### 侧边栏布局优化（macOS 兼容）
- **标题居中**：「AI Session Viewer」标题改为居中显示，避免与 macOS 左上角窗口控制按钮（红绿灯）重叠
- **左下角版本号**：Sidebar 底部左侧显示当前版本号，取代原来的"N 个项目"文字
- **更新检测移入设置**：桌面端的更新检查面板从 Sidebar 底部移入设置弹窗的「更新检查」选项卡，保持底栏简洁

---

## [1.9.2] - 2026-03-03

### Fixed

#### Docker 镜像 GLIBC 兼容性
- **根因**：Dockerfile 中 `rust:1` 基础镜像已升级到 Debian Trixie（GLIBC 2.39），而运行镜像 `debian:bookworm-slim` 仍是 GLIBC 2.36，导致编译出的二进制在运行镜像中无法启动
- **修复**：将编译镜像固定为 `rust:1-bookworm`，确保编译和运行环境 GLIBC 版本一致（2.36）

### Improved

#### CI Docker 构建加速
- CI 的 `docker` job 不再从源码完整编译 Rust，改为复用 `web-server` job 已编译的二进制（通过 Actions artifact 传递）
- Docker 镜像构建从数十分钟缩短到几秒，且确保镜像中的二进制与 Release 中上传的完全一致

---

## [1.9.1] - 2026-03-03

### Fixed

#### Codex 非交互式会话误显示
- **根因**：Codex 内部的 SubAgent、Exec、Mcp 等非交互式会话也出现在会话列表中，这些会话是 CLI 内部使用的，对用户无意义
- **修复**：解析 Codex session_meta 中的 `source` 字段，仅保留 `cli` 和 `vscode` 来源的交互式会话（与 Codex 自身的 `INTERACTIVE_SESSION_SOURCES` 一致）；`source` 缺失时默认视为交互式（兼容旧格式）

#### Resume 失败无前端反馈
- **根因**：当会话对应的项目路径已被删除时，后端返回"项目路径不存在"错误，但前端仅 `console.error` 静默吞掉，用户看不到任何反馈
- **修复**：会话列表页和消息详情页的 Resume 按钮均新增错误提示条（红色 toast），显示具体错误信息，5 秒后自动消失

#### Codex 会话 ID 提取不准确
- **根因**：当 Codex 会话文件缺少 session_meta 时，fallback 使用完整文件名（如 `rollout-2025-01-05T12-00-00-UUID`）作为 sessionId，导致 `codex resume` 无法识别
- **修复**：新增 `extract_uuid_from_filename()` 函数，从文件名末尾提取标准 UUID 格式的 36 字符 ID

---

## [1.9.0] - 2026-03-02

### Added

#### 会话分叉（Fork）
- 在消息详情页，hover 任意用户消息可看到三个操作按钮：Resume（恢复）、Fork（分叉）、Star（收藏）
- **Fork**：从选中的用户消息处分叉出一个全新会话，复制该消息及之前的所有 JSONL 内容到新文件（生成新 sessionId），自动注册到 `sessions-index.json`，并在系统终端中用 `claude --resume {新ID}` 打开
- **Resume**：直接在终端中恢复当前会话（与顶栏 Resume 按钮效果一致）
- 仅在 Tauri 桌面模式 + Claude 数据源下显示
- Fork 成功后按钮短暂变绿，后台自动刷新会话列表
- 后端新增 `fork_session_from_message()` 核心函数（`session-core`），逐行解析 JSONL 并替换 sessionId
- 后端新增 `fork_and_resume` Tauri 命令，重构终端启动代码为可复用的 `open_terminal()` 私有函数
- 前端 `tauriApi.ts` 新增 `forkAndResume()` API；`webApi.ts` 提供 no-op 实现

#### 终端类型选择（Windows）
- 设置面板新增「终端类型」选项，可切换 CMD 或 PowerShell
- 恢复会话和分叉会话时使用用户选择的终端类型
- 偏好持久化到 localStorage

---

## [1.8.0] - 2026-03-01

### Added

#### 收藏系统
- 支持收藏整个会话和会话中的具体用户消息
- 侧边栏新增「收藏」导航入口（Star 图标），显示当前数据源的收藏数量角标
- 会话列表页：每个会话卡片悬停时显示收藏星标按钮，已收藏时常驻显示
- 消息详情页：每条用户消息右侧悬停时显示收藏星标按钮，已收藏时常驻显示
- 收藏页面（`/bookmarks`）：按项目分组展示所有收藏项，区分会话级和消息级收藏
- 点击收藏项直接跳转到对应会话，消息级收藏自动滚动到目标消息并高亮闪烁 2 秒
- 收藏数据存储在 `~/.session-viewer-bookmarks.json`，原子写入（tmp + rename）
- 后端：`session-core/src/bookmarks.rs` 提供 load/save/add/remove/list 操作
- Tauri 命令：`list_bookmarks`、`add_bookmark`、`remove_bookmark`
- Web 路由：`GET /api/bookmarks`、`POST /api/bookmarks`、`DELETE /api/bookmarks/:id`

### Fixed

#### tool_result 消息误识别为用户消息
- Claude 的 `tool_result` 消息在 JSONL 中以 `role: "user"` 存储，但内容全是 `tool_result` 块
- 修复：解析时检查如果 `role == "user"` 且所有 content blocks 都是 `ToolResult` 类型，则将 role 改为 `"tool"`
- 这些消息现在正确渲染为左侧缩进的工具输出，timeline 导航点也不再将其误计为用户消息

### New Files

| 文件 | 说明 |
|------|------|
| `crates/session-core/src/bookmarks.rs` | 收藏核心逻辑（模型、存储、CRUD） |
| `src-tauri/src/commands/bookmarks.rs` | Tauri 收藏命令 |
| `crates/session-web/src/routes/bookmarks.rs` | Web 收藏路由 |
| `src/components/bookmark/BookmarksPage.tsx` | 收藏列表页面 |

---

## [1.7.1] - 2026-02-27

### Improved

#### 模型智能记忆与自动选择
- 模型选择器切换模型时自动持久化到 localStorage，下次打开应用立即恢复上次使用的模型
- ChatPage 和 QuickChatPage 页面加载时自动获取最新模型列表，不再依赖硬编码默认值
- 模型自动选择优先级：上次使用 > 用户设置默认 > CLI 配置默认 > 列表第一个
- 支持短名称模糊匹配（如 CLI 配置的 "opus" 自动匹配 "claude-opus-4-6"）

#### 历史会话续聊模型匹配
- 在历史会话页面续聊时，自动提取该会话使用的模型并预选（如会话用了 opus 则自动选中 opus）
- 不再固定为 claude-sonnet-4，尊重原始会话的模型选择

### Removed
- 移除 ChatPage 和 QuickChatPage 中的硬编码 `DEFAULT_MODEL` 常量

## [1.7.0] - 2026-02-27

### Added

#### 工具调用专用查看器
- 新增 6 种工具专用查看器，替代原有的纯 JSON 折叠展示：
  - **Read** — 语法高亮代码 + 行号，自动识别文件语言
  - **Edit** — LCS 算法 Diff 对比视图，显示添加/删除行数统计
  - **Write** — 语法高亮代码块，支持复制按钮
  - **Bash** — 终端风格展示（`$` 前缀命令 + 输出），错误高亮
  - **Grep/Glob** — 搜索参数 + 结果展示
  - **Default** — 通用 JSON 输入/输出（兜底）
- 工具调用链接：`tool_use` 与 `tool_result` 跨消息自动配对，在 tool_use 处统一展示输入和输出

#### 对话轮次分组
- 消息按对话轮次（Turn）自动分组，每轮以用户文本消息开始
- 轮次间显示分隔线和轮次编号
- 每轮显示该轮累计 token 用量

#### Token 用量详细展示
- 会话头部显示累计 token 明细：输入 / 输出 / 写入缓存 / 读取缓存
- 每条助手消息显示 token 分项（输入/输出/缓存），hover 查看完整明细
- 对话结束的 result 消息显示完整 token 分项，**不再显示价格**

#### 虚拟化滚动
- 使用 `@tanstack/react-virtual` 在对话轮次级别实现虚拟化
- 超过 30 轮对话时自动启用，低于阈值使用普通渲染
- 支持动态高度测量，适配可变内容

### Changed

#### 对话功能精简为 Claude 专属
- CLI 对话和快速问答移除所有 Codex 相关逻辑，简化为 Claude 专属
- 模型选择器、CLI 配置检测、流式输出均硬编码为 Claude
- 设置面板移除 Codex 配置区域
- **会话历史浏览**保留双数据源支持（Claude + Codex），不受影响

### New Files

| 文件 | 说明 |
|------|------|
| `src/components/chat/tool-viewers/DiffView.tsx` | LCS 算法 Diff 视图组件 |
| `src/components/chat/tool-viewers/ToolViewers.tsx` | 6 种工具专用查看器 |

---

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

[2.8.2]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v2.8.2
[2.8.1]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v2.8.1
[2.8.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v2.8.0
[2.7.2]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v2.7.2
[2.7.1]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v2.7.1
[2.2.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v2.2.0
[2.1.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v2.1.0
[1.9.7]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.9.7
[1.9.6]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.9.6
[1.9.4]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.9.4
[1.9.3]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.9.3
[1.9.2]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.9.2
[1.9.1]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.9.1
[1.9.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.9.0
[1.8.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.8.0
[1.7.1]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.7.1
[1.7.0]: https://github.com/zuoliangyu/AI-Session-Viewer/releases/tag/v1.7.0
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
