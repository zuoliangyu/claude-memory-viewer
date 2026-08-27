# Codex 轨迹视图

Codex 会话详情页在 `codex` 数据源下提供“轨迹”视图。它与现有消息视图共用同一个 rollout 文件，但使用独立的事件投影模型，不会改变 Claude、Grok 或既有 Codex 消息接口。

## 数据范围

轨迹投影器位于 `crates/session-core/src/provider/codex_trajectory.rs`，输出 `schemaVersion: 1` 的 `Trajectory`：

- `session`：会话元数据、模型、工作目录、归档状态和时间范围；
- `turns`：Turn 状态、耗时、首个模型响应耗时、模型调用数和 Token 汇总；
- `records`：用户消息、助手消息、Reasoning、命令与工具调用、Web 搜索、图片操作、文件变更、子 Agent、Review、Compaction 和完成事件；
- `pagination`：当前记录区间、前后剩余数量以及向前翻页 cursor；
- `warnings`：坏 JSONL 行、缺失或循环的 `history_base` lineage、未支持的完成事件。

Token 统计遵循 Codex 的累计快照：优先使用 `last_token_usage`，没有增量时由相邻 `total_token_usage` 做差；缓存输入和推理输出是输入/输出的子集，不应再次相加。

## Rollout 兼容性

支持 legacy 单文件 JSONL 和带 `history_mode: "paginated"` 的 `history_base` 分段。分页 lineage 按 `end_ordinal_exclusive` 与 `end_byte_offset` 合并，过滤继承上下文，避免同一事件重复展示。归档文件可通过当前会话路径直接读取。

解析过程只读本地文件，完整输出字段统一限制在 12,000 字符以内；未知事件不会阻断会话，其信息会进入警告列表。

首次打开采用两阶段加载。大于 8 MiB 的当前 rollout 会先从文件尾部读取最多 8 MiB，返回最近事件并结束首屏加载；随后在阻塞线程池中完整投影当前 rollout lineage，补全全局统计、稳定序号和早期分页。快速结果的 `pagination.complete` 为 `false`，其中 Turn、Token、工具数和记录数只代表当前片段；完整结果会整体替换快速结果，不会混合两套序号。8 MiB 以内的文件直接返回完整投影。

完整投影按 JSONL 逐行处理，而不是同时保留整份事件树，并按文件修改时间与大小缓存最近两个会话；切换视图和加载更早记录会复用缓存。相同文件的并发缓存缺失会合并为一次构建，避免 React 开发模式重复扫描超大文件；不同文件仍可独立投影。lineage 查找优先检查相邻目录，必要时才建立一次进程级 rollout 索引。

## API

桌面端使用 Tauri command：`get_trajectory(source, filePath, maxRecords?, beforeRecord?, fast?)`。

Web 端使用：

```text
GET /api/trajectory?source=codex&filePath=<url-encoded-rollout-path>&maxRecords=500&beforeRecord=<exclusive-record-index>
```

当前轨迹接口只接受 `codex` 数据源。`fast=true` 请求快速尾部页；未传或为 `false` 时请求完整投影。`maxRecords` 默认 500，服务端限制为 50 到 1000；不传 `beforeRecord` 时返回最近一页，继续向前加载时传入上一页的 `pagination.nextBeforeRecord`。完整页中 `record.index` 是稳定序号，相邻页不会重叠；`stats.records` 表示全会话记录总数，`stats.visibleRecords` 表示当前响应条数。快速页的序号和统计只在当前片段内有效，不能直接与完整页合并。

前端每页加载 80 条记录，按稳定序号合并早期页面，并支持在已加载范围内搜索 `event`、`summary`、`input`、`output`，以及按记录类型筛选。轨迹模式使用带布局/绘制隔离的独立原生滚动容器，不触发消息分页、位置百分比和滚动按钮状态计算；后台完整结果通过低优先级 React transition 提交，耗时轴与 Turn 区块复用未变化的渲染结果。Turn 默认折叠，离屏区块使用 `content-visibility` 延迟布局和绘制，避免大轨迹首屏一次挂载过多记录行。后续如要支持实时尾部更新，应在此模型上增加 revision/cursor，而不是复用消息分页接口。

## 开发性能诊断

通过 `./dev.ps1` 启动桌面应用时会自动启用性能诊断，并将终端输出同时保存到 `target/perf/dev-<时间>.log`。直接运行 `npx tauri dev`、Web 开发模式和正式构建均不会启用该诊断。Rust 端还使用 `debug_assertions` 做二次限制，正式构建即使被设置同名环境变量也不会接受前端性能事件。

复现一次卡顿后可运行 `./scripts/analyze-perf-log.ps1` 分析最新日志。脚本默认将最慢消息 IPC 超过 5 秒视为失败，也可通过 `-IpcThresholdMs` 调整阈值；它只读取开发日志，不会在正式构建中运行。

复现卡顿后，可用以下命令提取时间线：

```powershell
Select-String -Path target\perf\dev-*.log -Pattern '\[ASV-PERF\]'
```

每行在 `[ASV-PERF]` 后输出一个 JSON 事件，不包含会话正文或完整文件路径。主要事件如下：

- `trajectory.backend_parse`：Rust 轨迹投影和分页耗时。
- `messages.backend_parse`：消息分页扫描耗时。Tauri 桌面端在阻塞线程池中执行该扫描，因此大型会话的消息解析不会再阻塞轨迹命令。
- `messages.range_backend_parse`：消息窗口区间扫描耗时，用于定位前后翻页触发的全量解析。
- `messages.request_started` / `messages.ipc_roundtrip`：消息请求开始和完整 IPC 往返耗时，并记录消息数、内容块数和近似文本体积。
- `messages.store_applied` / `messages.store_skipped`：消息快照是否实际写入 Zustand；相同快照和 StrictMode 重复请求会被跳过。
- `messages.react_commit`：React Profiler 测得的消息树提交耗时。
- `messages.dom_committed` / `messages.paint_ready`：消息响应到 DOM 提交、以及后续可绘制的耗时，同时携带 DOM 节点数和 JS 堆内存。
- `watcher.refresh_dispatched` / `messages.refresh_skipped`：文件监听合并后的路径数量，以及因变更不属于当前会话而跳过消息刷新的记录。
- `background_refresh.completed`：项目与会话后台刷新耗时，并标记返回数据是否实际改变 store。
- `stats.session_cost_backend`：当前会话账单的后端读取耗时；Codex 只扫描当前 rollout，不再为一个徽标遍历全部会话。
- `trajectory.ipc_roundtrip`：前端调用到收到完整结果的总耗时，包含后端解析、序列化、IPC 传输和前端反序列化。
- `react.commit`：React Profiler 测得的轨迹子树渲染耗时。
- `trajectory.dom_committed`：收到结果到 DOM 提交完成的等待时间，并携带 DOM 节点数和 JS 堆内存。
- `trajectory.paint_ready`：DOM 提交后经过两个动画帧的可绘制时间。
- `browser.long_task`：WebView 主线程中超过 50ms 的长任务。
- `browser.event_loop_lag`：可见窗口中的事件循环延迟超过 100ms。

`fields` 中的 `detailChars`、`textChars` 和 `approximateTextMb` 用于判断大段 `input/output` 或消息正文是否造成 IPC 与内存压力；`documentNodes`、`rootNodes` 和 `usedHeapMb` 用于判断 DOM 与垃圾回收压力。轨迹请求在 React StrictMode 下仍可能出现两次；消息首次请求通过 in-flight 合并，只会执行一次后端读取，并以 `messages.request_deduplicated` 记录被合并的调用。

桌面端的项目与会话扫描命令统一在阻塞线程池执行，避免深扫大型 rollout 时占用 Tauri UI/IPC 线程。启动后的首次后台核对只读取现有缓存，不会立即重复强制扫描；手动重建和周期刷新仍会更新缓存，但不会阻塞窗口交互。开发诊断的终端写入同样在阻塞线程执行，日志管道变慢时不会反向占用 UI 线程。

消息页的会话账单按文件读取，并对相同数据源与文件的并发请求做 singleflight 合并。React StrictMode 重复挂载不会再触发两次 Codex 全库账单扫描。

## 归属

本功能参考 [icesixgod/codex-trajectory](https://github.com/icesixgod/codex-trajectory) 的公开事件投影与 `history_base` 语义，未引入其 Python runtime、MCP 代码或 UI 资产；该项目采用 MIT License，完整声明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
