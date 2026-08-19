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

首次打开需要完整投影当前 rollout lineage，以保证全局统计与稳定序号正确。投影在阻塞线程池执行，JSONL 逐行处理而不是同时保留整份事件树，并按文件修改时间与大小缓存最近两个会话；切换视图和加载更早记录会复用缓存。lineage 查找优先检查相邻目录，必要时才建立一次进程级 rollout 索引。

## API

桌面端使用 Tauri command：`get_trajectory(source, filePath, maxRecords?, beforeRecord?)`。

Web 端使用：

```text
GET /api/trajectory?source=codex&filePath=<url-encoded-rollout-path>&maxRecords=500&beforeRecord=<exclusive-record-index>
```

当前轨迹接口只接受 `codex` 数据源。`maxRecords` 默认 500，服务端限制为 50 到 1000；不传 `beforeRecord` 时返回最近一页，继续向前加载时传入上一页的 `pagination.nextBeforeRecord`。`record.index` 是完整投影生成的稳定序号，相邻页不会重叠；`stats.records` 始终表示全会话记录总数，`stats.visibleRecords` 表示当前响应条数。

前端每页加载 200 条记录，按稳定序号合并早期页面，并支持在已加载范围内搜索 `event`、`summary`、`input`、`output`，以及按记录类型筛选。后续如要支持实时尾部更新，应在此模型上增加 revision/cursor，而不是复用消息分页接口。

## 归属

本功能参考 [icesixgod/codex-trajectory](https://github.com/icesixgod/codex-trajectory) 的公开事件投影与 `history_base` 语义，未引入其 Python runtime、MCP 代码或 UI 资产；该项目采用 MIT License，完整声明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
