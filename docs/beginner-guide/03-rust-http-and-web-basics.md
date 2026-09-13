# 03. Rust、HTTP 与 Web

管理路由位于 `/api/v2/sunshine/*`，只接受 Foundation 管理员 Session；写请求还需要 CSRF 与同源验证。
Client 路由位于 `/sunshine-client/v1/*`，不接受浏览器 Cookie/Origin，只信任同机 TLS ingress 提供的 HTTPS
事实。两组身份域不能互换。

所有 JSON DTO 拒绝未知字段并限制 64 KiB 消息。HTTP handler 只验证、查询或持久化任务；它不建立到
Sunshine 的网络连接。WSS 将任务交给已认证 Client，Client 结果再驱动 operation 状态。

React Web 有三个产品页：实例列表、实例详情、日志。它必须用 API validator 解析 Server 投影，不能把
按钮点击或 202 当成 Sunshine 已完成操作。语言切换直接发生，不显示丢失编辑确认框。
