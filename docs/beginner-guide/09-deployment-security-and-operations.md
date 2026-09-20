# 09. 部署、安全与生产运维

Server 只部署到 Linux AMD64 的不可变 release 树，以专用账户运行并监听 loopback。可信 HTTPS/WSS ingress
是唯一外部入口；它必须保留浏览器同源事实，并为独立 Client 通道提供已验证 HTTPS 事实。

监控 readiness、数据库/WAL、operation backlog/unknown、Client 在线和 Sunshine 可达性。pending 通常
检查 Client/WSS；unknown 必须核对 Sunshine 与 Client 执行日志，禁止盲重试重启或配置写入。

数据库必须与 credential key 独立保全，但当前产品及 `sarmg-upgrade` 都没有 0.11.3 的受支持备份恢复或
key rotation。不能用旧 0.8.0 适配器。Secret 泄露时隔离服务、撤销管理员会话和实例 credential，并在
已有受审转换能力前建立全新当前状态。

公开问题不得附带授权码、Client credential、管理员密码、数据库、生产 URL 或 Sunshine 凭据。
