# 能力地图

> 位置：项目根 `.ai-governance/capability-map.md`
> 用途：G2 门禁的复用核对基准。所有条目均为本次静态核对结果，登记日期为 2026-07-29。

## 一、前端组件 / 页面模式

| 能力 | 复用入口 | 适用场景 | 不适用 | 登记日期 |
|------|----------|----------|--------|----------|
| 数据源切换与会话页面 | `src/stores/appStore.ts:175`、`src/components/session/SessionsPage.tsx:44` | 新本地会话源需要项目、会话、消息页面共用状态与 UI | 只读源缺少统一 `source` 标识时 | 2026-07-29 |
| 会话导出交互 | `src/components/session/SessionsPage.tsx:218` | 已由统一 API 返回导出内容的本地会话源 | Grok 格式无法转换为统一消息模型时 | 2026-07-29 |
| 会话恢复交互 | `src/components/session/SessionsPage.tsx:154`、`src/components/message/MessagesPage.tsx:910` | CLI 支持以稳定会话 ID 恢复时 | Grok CLI 不支持恢复或恢复参数语义不同且无法适配时 | 2026-07-29 |

## 二、共享状态 / hooks / 工具函数

| 能力 | 复用入口 | 适用场景 | 不适用 | 登记日期 |
|------|----------|----------|--------|----------|
| 双端统一 API 门面 | `src/services/api.ts:9`、`src/services/tauriApi.ts:32`、`src/services/webApi.ts:207` | Tauri IPC 与 Web HTTP 同时接入同一会话源 | 新能力仅有单端且不可提供对等接口时 | 2026-07-29 |
| 后台刷新与竞态保护 | `src/stores/appStore.ts:278`、`src/stores/appStore.ts:775` | 文件变化后刷新项目、会话和当前消息 | 刷新须改为推送增量数据时 | 2026-07-29 |
| 文件变化订阅 | `src/hooks/useFileWatcher.ts:10` | 新源可接入既有 Tauri 事件 / WebSocket 文件变化通知 | 数据源不在本地文件系统时 | 2026-07-29 |

## 三、后端 Service / Mapper

| 能力 | 复用入口 | 适用场景 | 不适用 | 登记日期 |
|------|----------|----------|--------|----------|
| Provider 分发 | `crates/session-core/src/provider/mod.rs:1`、`src-tauri/src/commands/projects.rs:6` | 新会话源实现共享模型并接入 source 分发 | 会话源不能映射为项目/会话/消息模型时 | 2026-07-29 |
| Codex JSONL Provider 范式 | `crates/session-core/src/provider/codex.rs:299`、`crates/session-core/src/provider/codex.rs:1066`、`crates/session-core/src/provider/codex.rs:1218` | 本地按目录发现、索引、分页解析 JSONL 会话 | Grok 存储结构或记录格式不兼容时 | 2026-07-29 |
| 导出渲染 | `crates/session-core/src/export.rs:36` | 新源可解析为 `DisplayMessage` 后导出 JSON / Markdown / HTML | Grok 数据不能安全映射到统一消息模型时 | 2026-07-29 |

## 四、权限 / 审计 / 异常 / 日志

| 能力 | 复用入口 | 适用场景 | 不适用 | 登记日期 |
|------|----------|----------|--------|----------|
| 会话文件路径校验 | `crates/session-core/src/paths.rs:96` | 从 UI 传入本地会话文件路径的读取、导出、恢复等操作 | 新源不以文件路径定位会话时 | 2026-07-29 |
| WebSocket 鉴权与重连监听 | `crates/session-web/src/ws.rs:28`、`crates/session-web/src/ws.rs:136` | Web 模式订阅本地会话文件变化 | 不使用 WebSocket 的交付模式 | 2026-07-29 |

## 五、数据库脚本 / 测试数据

| 能力 | 复用入口 | 适用场景 | 不适用 | 登记日期 |
|------|----------|----------|--------|----------|
| 不涉及 | 用户确认范围仅为 Grok CLI 本地会话接入 | 本次不新增或迁移数据库 | 后续另行要求持久化索引或数据迁移 | 2026-07-29 |

## 六、文档模板 / 流程文档

| 能力 | 复用入口 | 适用场景 | 不适用 | 登记日期 |
|------|----------|----------|--------|----------|
| G2 实现前设计 | `.ai-governance/grok-cli-local-session-design.md:1` | 本次 Grok CLI 本地会话接入进入实现前的范围、方案和验收约束 | 已完成实施后的 G3 验收归档 | 2026-07-29 |

## 七、已知禁区

| 禁区 | 说明 | 正确做法 | 登记日期 |
|------|------|----------|----------|
| Grok 网页/API 历史 | 用户明确排除云端历史读取 | 只读取 Grok CLI 的本地会话数据 | 2026-07-29 |
| 写入型会话操作 | 用户明确排除发送、删除、回收站 | 本次仅设计发现、读取、导出、恢复、监听刷新 | 2026-07-29 |
| 扩展统计或搜索 | 用户明确排除统计、搜索 | 保持新源不进入统计、搜索分发 | 2026-07-29 |
