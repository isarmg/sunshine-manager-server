# 02. 环境与第一次运行

Server 开发需要固定 Rust/Node 工具链、SQLite URL、绝对 Web dist、32 字节 Base64 credential key、
bootstrap 管理员密码和 loopback bind。使用 `config/sunshine-manager.env.example`，不要提交真实秘密。

先构建 Web，再运行 Rust 检查。Server 启动后检查 `/healthz`、`/readyz` 和管理员登录。创建一个测试实例，
再在隔离主机运行独立 Sunshine Client 完成配对；不要把开发 Client 指向已有生产 Sunshine。

常见失败分层：Server TLS/Origin、实例授权码或已轮换、Client WSS ingress、Client 本地证书验证、
Sunshine 认证、配置 revision 冲突。Server 无法替 Client 验证本机 Sunshine 密码。
