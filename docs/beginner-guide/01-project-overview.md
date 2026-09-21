# 01. 项目全景

Sunshine Manager 是 Sunshine 的集中控制面，由 Linux AMD64 Server、管理 Web、独立跨平台 Client 和严格
协议组成。Moonlight 仍直接连接 Sunshine；Manager 不转发游戏画面。

Server 保存管理员、实例、长期授权码、Client credential 摘要、配置快照、任务和审计。Client 保存本机
Sunshine endpoint、用户名/密码和执行日志。两端之间只有当前
`sunshine-management/2` 协议。

当前软件是 0.11.7，管理 API 为 `/api/v2`，数据库为 revision 7。应用与 Moonlight 管理由 Client 调用
本机固定 Sunshine API 完成，Server 不保存 Sunshine 密码。

主要入口：`src/http.rs` 看路由与 WSS，`src/db.rs` 看实例，`src/operations.rs` 看持久任务，`protocol/`
看消息与字段白名单，`web/src/` 看实例列表、详情和日志。
