# Sunshine Manager 0.11.2 初学者指南

本指南描述当前 Server + 独立 Client 架构。先理解实例和 WSS 通道，再理解配置任务与不确定性：

1. [项目全景](01-project-overview.md)
2. [环境与第一次运行](02-environment-and-first-run.md)
3. [Rust、HTTP 与 Web](03-rust-http-and-web-basics.md)
4. [认证与请求生命周期](04-authentication-and-request-lifecycle.md)
5. [持久任务与恢复](05-durable-operations-and-recovery.md)
6. [实例与 Client 管理](06-instance-and-client-management.md)
7. [Client 协议与配置边界](07-client-protocol-and-current-contracts.md)
8. [测试、调试与变更](08-testing-debugging-and-change-workflow.md)
9. [部署、安全与运维](09-deployment-security-and-operations.md)
10. [阅读路线与术语](10-reading-roadmap-and-glossary.md)

最重要的边界：Server 不保存 Sunshine 用户名/密码或证书，不直接调用 Sunshine，也不参与视频串流。
独立 Client 在 Sunshine 主机上通过回环 HTTPS 管理本机 Sunshine，并以 HTTPS/WSS 主动连接 Server。
