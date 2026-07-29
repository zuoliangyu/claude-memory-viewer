# Grok CLI 本地会话接入（实现前设计）

## 文档定位

- 类型：实现前设计（G2）
- 来源需求：Grok CLI 本地会话的发现、读取、导出、恢复、监听刷新，以及桌面/Web/UI 接入。
- 关联任务：本次 Grok CLI 本地会话接入。
- 复杂度：大（G1 结论由任务上下文提供：命中 H1、H3、H6、H8、H10）。
- 状态：已确认 / 实施中
- 最后更新：2026-07-29

## 一、当前理解

在不访问 Grok 网页或 API 历史的前提下，新增一个只读的本地 Grok CLI 会话源。该源须按 Grok CLI 的本地持久化格式发现会话、解析为现有统一项目/会话/消息模型，并复用已有桌面 IPC、Web HTTP/WebSocket、会话导出、恢复入口和刷新机制。用户明确排除聊天发送、删除、回收站、统计和搜索。

范围依据：任务上下文中的用户确认：“Grok CLI 本地会话的发现、读取、导出、恢复、监听刷新、桌面/Web/UI 接入；明确不做 Grok 网页/API 历史、聊天发送、删除/回收站、统计、搜索。”

## 二、复杂度判定

> 本表记录已提供的 G1 结论；本 G2 文档不重新降低复杂度。

| 硬条件 | 命中 | 依据 |
|--------|------|------|
| H1 涉及文件 ≥5 | 是 | 用户提供的 G1 结论“命中 H1”；推荐方案至少涉及 Provider、核心路径校验、Tauri 命令、Web 路由、双端 API、store/UI 与监听分发，现有同类入口见 `src-tauri/src/commands/projects.rs:6`、`src/services/tauriApi.ts:32`、`src/services/webApi.ts:207`、`src/stores/appStore.ts:278`、`crates/session-web/src/ws.rs:28`。 |
| H2 改数据库结构 | 否 | 用户确认范围未包含数据库；本次范围依据见本文“一、当前理解”。 |
| H3 新增/改对外接口 | 是 | 用户提供的 G1 结论“命中 H3”；现有 Tauri 命令按 `source` 分发（`src-tauri/src/commands/projects.rs:6`），Web 项目接口接受 `source`（`src/services/webApi.ts:207`），新增 `grok` 取值将扩展两端契约。 |
| H4 涉及权限/鉴权 | 否 | 不新增权限模型；现有 WebSocket 鉴权保持复用（`crates/session-web/src/ws.rs:136`）。 |
| H5 涉及资金/订单/库存 | 否 | 用户确认的本地会话读取范围不含资金、订单或库存。 |
| H6 改公共模块 | 是 | 用户提供的 G1 结论“命中 H6”；共享 Provider 模块当前仅导出 Claude/Codex（`crates/session-core/src/provider/mod.rs:1-2`），统一 API 门面被两端调用（`src/services/api.ts:9`）。 |
| H7 引入新依赖 | 否 | 推荐方案复用已有 Rust/前端能力；当前阶段不计划新增依赖。 |
| H8 跨端协同 | 是 | 用户提供的 G1 结论“命中 H8”；桌面经 Tauri API（`src/services/tauriApi.ts:32`），Web 经 HTTP API（`src/services/webApi.ts:207`），前端以 `src/services/api.ts:9` 选择实现。 |
| H9 改构建/部署/CI | 否 | 本次范围不含构建、部署或 CI；仓库指引将三者列为独立发布流程（`AGENTS.md:75`）。 |
| H10 业务口径不明 | 是 | 用户提供的 G1 结论“命中 H10”；Grok CLI 本地会话根目录、文件格式、项目归属与恢复命令参数尚未提供可复核样本或官方本地格式证据。 |

- 命中条数：5（H1、H3、H6、H8、H10）。
- 判定结果：大。
- 是否存在用户确认降级：否。
- 降级后跳过的治理项：不适用。
- 降级后剩余风险：不适用。

## 三、现状调查结论

| 调查项 | 结论 | 依据 |
|--------|------|------|
| 相关页面 | UI 已按 `source` 读取会话并提供导出与恢复入口。 | `src/components/session/SessionsPage.tsx:44`、`src/components/session/SessionsPage.tsx:154`、`src/components/session/SessionsPage.tsx:218`。 |
| 相关接口 | Tauri 与 Web 已有项目、会话、导出接口，均以 `source` 调度。 | `src-tauri/src/commands/projects.rs:6`、`src-tauri/src/commands/sessions.rs:27`、`src-tauri/src/commands/export.rs:7`；`src/services/webApi.ts:207`、`src/services/webApi.ts:219`、`src/services/webApi.ts:331`。 |
| 相关数据库 | 本次尚未发现需要修改的数据库脚本；用户范围不含数据库。 | 本次只读检索命令 `rg -n -S "claude|codex|cursor|session|transcript|watch|watcher|export" src src-tauri crates package.json Cargo.toml` 的结果聚焦文件 Provider/API/UI；用户确认范围见本文“一、当前理解”。 |
| 相关权限 | 会话路径校验由核心模块集中实施；WebSocket 文件变化订阅已有鉴权入口。 | `crates/session-core/src/paths.rs:96`；`crates/session-web/src/ws.rs:136`。 |
| 相关文档 | 项目架构说明已定义 Claude/Codex 双数据源、统一 API 与文件监听。 | `AGENTS.md:38`、`AGENTS.md:44`、`AGENTS.md:51`。 |
| 相关已有实现 | Codex Provider 已具备本地目录发现、项目/会话读取、分页消息解析、缓存失效；其可作为 Grok Provider 的结构范式，但不能直接解析未知 Grok 格式。 | `crates/session-core/src/provider/codex.rs:299`、`crates/session-core/src/provider/codex.rs:888`、`crates/session-core/src/provider/codex.rs:1066`、`crates/session-core/src/provider/codex.rs:1131`、`crates/session-core/src/provider/codex.rs:1218`。 |

未确认：Grok CLI 的本地根目录、会话文件格式、项目归属字段、稳定会话 ID，以及恢复命令/参数；静态仓库代码不包含这些事实，实施前须以用户提供的脱敏样本或本机只读探测确认。

## 四、能力地图核对

已读取并初始化 `.ai-governance/capability-map.md:1`。

| 能力类型 | 是否已有可复用能力 | 复用入口（具体路径） | 本次处理 | 若新增，为何不能复用 |
|----------|--------------------|---------------------|----------|---------------------|
| 前端组件 / 页面模式 | 是 | `src/components/session/SessionsPage.tsx:44` | 扩展 | 复用现有 source 驱动页面；仅补充 Grok 显示文案/分支。 |
| 共享状态 / hooks / 工具函数 | 是 | `src/services/api.ts:9`、`src/stores/appStore.ts:278`、`src/hooks/useFileWatcher.ts:10` | 扩展 | 复用双端 API 门面、状态刷新和监听；仅扩展 source 枚举与调用分发。 |
| 后端 Service / Mapper | 部分 | `crates/session-core/src/provider/codex.rs:299`、`crates/session-core/src/export.rs:36` | 新增 + 复用 | 必须新增 Grok Provider/格式解析，因为 Codex Provider 的根目录、四层年月日布局与 `rollout-` 文件名校验是 Codex 专用（`crates/session-core/src/paths.rs:61`）；导出渲染可复用。 |
| 权限 / 审计 / 异常 / 日志 | 是 | `crates/session-core/src/paths.rs:96`、`crates/session-web/src/ws.rs:28` | 扩展 | 复用路径规范化、根目录限制、WebSocket 鉴权和监听；新增 Grok 根目录/布局校验分支。 |
| 数据库脚本 / 测试数据 | 否 | 不涉及 | 不涉及 | 用户范围不含数据库；不建立持久化索引。 |
| 文档模板 / 流程文档 | 是 | `.ai-governance/capability-map.md:1` | 复用 | 本文为 G2 设计产物。 |

## 五、设计方案

### 5.1 推荐方案

以“独立 Grok Provider + 统一 source 扩展”接入：

1. 先用本机只读探测和脱敏样本确认 Grok CLI 根目录、记录格式、项目分组和恢复标识；将确认结果写入实现任务证据。
2. 在 `session-core` 新增只读 Grok Provider，输出既有 `ProjectEntry`、`SessionIndexEntry`、`DisplayMessage`，并实现与现有 Provider 同级的发现、列表、分页读取和局部缓存失效。
3. 扩展集中路径校验，使 Grok 会话文件只能从已确认的 Grok 根目录与布局读取；复用 `render_session` 统一导出 JSON、Markdown、HTML。
4. 扩展 Tauri commands、Web routes、`tauriApi.ts`、`webApi.ts` 和前端 source 类型/文案，让同一 UI 同时在桌面和 Web 读取、导出、恢复 Grok 会话。
5. 将已确认 Grok 根目录加入现有 Tauri/Web 文件监听；沿用 1 秒防抖、局部缓存失效和前端后台刷新。
6. 恢复只在 Grok CLI 已确认支持且会话稳定 ID/参数被验证时启用；未验证时明确显示不可用，不以猜测命令执行外部进程。

### 5.2 不采用的方案

| 方案 | 核心做法 | 不采用原因 |
|------|----------|------------|
| 读取 Grok 网页/API 历史 | 调用云端接口或抓取网页历史 | 用户明确排除；会扩大鉴权、隐私和网络依赖范围。 |
| 将 Grok 伪装成 Codex Provider | 复用 Codex 根目录、`rollout-*` 命名和 JSONL 解析实现 | `crates/session-core/src/paths.rs:61` 将 Codex 布局固定为年月日/`rollout-`；Grok 格式未确认，强行共用会产生错误发现或越界路径校验。 |
| 为 Grok 新建独立页面与独立 API | 复制项目、会话、消息和导出 UI/接口 | 已有统一 source、API 门面和会话页面（`src/services/api.ts:9`、`src/components/session/SessionsPage.tsx:44`）；复制会造成桌面/Web 行为漂移。 |
| 将监听改为固定轮询 | 定时全量扫描本地目录 | 已有事件监听、1 秒防抖和局部失效（`crates/session-web/src/ws.rs:28`、`src/hooks/useFileWatcher.ts:10`）；轮询增加 I/O 且不必要。 |

### 5.3 复用与新增边界

- 本次复用：共享模型、API 门面、会话页面、导出 `render_session`、Tauri/Web 传输、文件变化通知、后台刷新、路径校验框架。
- 本次扩展：`source` 枚举与分发、路径校验的根目录/布局分支、监听根目录注册、UI 标签与恢复可用性判断。
- 本次新增：Grok 本地会话 Provider、Grok 格式解析器、Grok 专属路径规则，以及与已确认 Grok CLI 恢复语义相匹配的只读恢复参数适配。新增理由：已有 Claude/Codex Provider 的持久化格式和路径规则均为专用实现，不能安全覆盖未知 Grok 格式。
- **明确不做：** Grok 网页/API 历史、聊天发送、删除、回收站、统计、搜索、数据库迁移、构建/部署/CI 变更，以及在未验证恢复参数前执行 Grok CLI。

## 六、影响范围

| 范围 | 是否影响 | 具体对象与说明 |
|------|----------|----------------|
| 前端页面 | 是 | `src/components/session/SessionsPage.tsx`、`src/components/message/MessagesPage.tsx`、侧边栏数据源文案；复用页面，扩展 Grok source。 |
| 后端接口 | 是 | Tauri `src-tauri/src/commands/*` 和 Web `crates/session-web/src/routes/*` 的 source 分发；不新增云端接口。 |
| 数据库 | 否 | 不持久化 Grok 索引，不改表或迁移。 |
| 权限 | 是 | 扩展 `crates/session-core/src/paths.rs` 的本地根目录和布局校验；复用现有 WebSocket 鉴权。 |
| 脚本 / 配置 | 否 | 不新增依赖，不改构建/部署配置。 |
| 项目文档 | 是 | 维护本能力地图和本实现前设计；实现后按 G3 归档。 |
| API 文档 | 是 | source 参数可接受值新增 `grok` 时，更新 API/类型注释。 |
| 流程文档 | 是 | 实现前需把本设计与可复核样本结论关联到实现任务。 |

## 七、风险与回滚

| 风险 | 触发条件 | 影响 | 缓解措施 |
|------|----------|------|----------|
| Grok 本地格式判断错误 | 根目录/JSON 字段仅凭猜测实现 | 发现不到会话、解析错误或显示错误项目 | 先获取脱敏样本和只读目录证据；为解析器增加样本测试和清晰错误。 |
| 路径越界读取 | Grok 路径校验未限制根目录或符号链接后路径 | 可能读取非会话文件 | 复用 `canonicalize`/根目录/布局校验模式（`crates/session-core/src/paths.rs:96`），为 Grok 单独测试越界和错误扩展名。 |
| 监听漏报或重复刷新 | 文件事件布局与既有过滤条件不匹配 | UI 陈旧或重复 I/O | 把已确认根目录接入同一 watcher，验证创建、追加、重命名事件和 1 秒防抖。 |
| 恢复误调用 | Grok CLI 恢复参数未验证 | 启动错误命令或恢复错误会话 | 未取得 CLI 帮助/样本证据时禁用恢复；验证稳定 ID 与命令参数后才开放。 |
| 两端契约漂移 | 仅改 Tauri 或仅改 Web source 分发 | 一个运行模式不可用 | 每个 API 用例同时覆盖 Tauri command 和 Web route/API adapter。 |

- **回滚方案：** 本次不命中 H2/H4/H5。实现阶段将以单独 Grok Provider 与 source 分支隔离；移除 `grok` 分发和监听注册即可停用，不触碰既有 Claude/Codex 会话文件。

## 八、验收标准

| 验收项 | 验证命令 / 操作 | 期望结果 | 实际结果 | 状态 |
|--------|----------------|----------|----------|------|
| 发现与读取 | 用脱敏 Grok CLI 本地样本执行 Provider 单元测试，并在桌面与 Web 选择 Grok source | 项目、会话、分页消息与样本一致；Claude/Codex 不回归 | 待实施 | ⬜ |
| 路径安全 | 单元测试空路径、非 `.jsonl`（或经确认的 Grok 后缀）、根目录外路径、符号链接越界和错误布局 | 所有非法路径被拒绝；合法 Grok 文件被接受 | 待实施 | ⬜ |
| 导出 | 从桌面和 Web 调用 Grok 会话导出 JSON/Markdown/HTML | 三种格式均含统一消息内容；不读取根目录外文件 | 待实施 | ⬜ |
| 恢复 | 以已确认 Grok CLI 版本和真实/脱敏恢复样本执行恢复 | 使用正确稳定会话 ID 恢复；未支持时 UI 禁用并说明 | 待实施 | ⬜ |
| 监听刷新 | 新建/追加/修改 Grok 本地会话文件后观察桌面事件和 WebSocket | 防抖后刷新受影响项目/会话；无全量无关刷新 | 待实施 | ⬜ |
| 双端契约 | 分别调用 Tauri command 与 Web HTTP/WebSocket 路径 | 两端均接受 `grok` source，错误格式返回一致且可读的失败 | 待实施 | ⬜ |
| 文档验收 | 复核本文件和能力地图 | 复用/新增/不做、风险、证据与范围齐全 | 已完成 G2 文档 | ✅ |

## 九、实现前结论

- [x] 可以进入实现。用户于本次会话回复：“继续”。
- [x] 需要先补充调查。
- [x] 需要用户确认业务口径。
- [ ] 需要拆分成多个任务。
- [ ] 用户已确认降级为轻量流程，并已记录跳过项和剩余风险。

## 十、待用户确认（阻塞点）

- 推荐方案：独立 Grok Provider 接入既有 source、双端 API、导出、恢复和文件监听框架；先核实本地格式与恢复参数。
- 复用：统一模型/API、会话 UI、导出渲染、路径校验框架、Tauri/Web 文件监听和后台刷新。
- 新增（含理由）：Grok Provider、解析器和路径规则；Grok 本地格式未知，不能复用 Claude/Codex 的专用持久化布局。
- 明确不做：网页/API 历史、发送、删除/回收站、统计、搜索、数据库/构建/部署/CI 改动。
- 主要风险：本地格式、根目录、项目归属和恢复参数尚无 A 级样本证据；路径校验和双端行为必须独立验证。
- 待确认口径：提供 Grok CLI 版本与 `--help`/恢复命令证据、脱敏的本地会话目录树及至少一份会话样本；确认“恢复”是唤起本地 Grok CLI 还是仅复制/打开会话标识。

**是否确认先完成上述只读格式调查，再进入实现？**
