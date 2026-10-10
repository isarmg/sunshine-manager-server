# 02. 环境与第一次运行

Server 开发需要固定 Rust/Node 工具链、SQLite URL、32 字节 Base64 credential key、
bootstrap 管理员密码和 loopback bind。使用 `config/xscs.env.example`，不要提交真实秘密。

先通过 xcss `xcss-build-server --config xcss-web-build.json --mode development` 构建 Web、Rust 并验证二进制资源清单，再运行 Rust 检查。Server 启动后检查 `/healthz`、`/readyz` 和管理员登录。创建一个测试实例，
再在隔离主机运行独立 xscc 完成配对；不要把开发 Client 指向已有生产 Sunshine。

常见失败分层：Server TLS/Origin、实例授权码或已轮换、Client WSS ingress、Client 本地证书验证、
Sunshine 认证、配置 revision 冲突。Server 无法替 Client 验证本机 Sunshine 密码。

开发可显式设置绝对 `XCSS_DEV_WEB_DIR` 配合 `PRODUCTION=false`，重新构建 Web 后无需重编译 Rust；生产拒绝目录资源，使用内嵌资源与精确绑定的 `web-assets.json`。
