# 09. 部署、安全与生产运维

Server 只部署到 Linux AMD64 的不可变 release 树，以专用账户运行并监听 loopback。可信 HTTPS/WSS ingress
是唯一外部入口；它必须保留浏览器同源事实，并为独立 Client 通道提供已验证 HTTPS 事实。

监控 readiness、数据库/WAL、operation backlog/unknown、Client 在线和 Sunshine 可达性。pending 通常
检查 Client/WSS；unknown 必须核对 Sunshine 与 Client 执行日志，禁止盲重试重启或配置写入。

数据库必须与 credential key 独立保全。产品进程不提供备份恢复或 key rotation；`sarmg-upgrade` 对当前
`0.10.1` / revision 7 数据库身份提供 keyed SQLite backup/verify/restore，使用前须核对支持矩阵并
验证隔离恢复。当前没有 Sunshine recover、key rotation 或跨 Schema 升级边。Secret 泄露时隔离服务、
撤销管理员会话和实例 credential；数据库主 key 泄露时建立全新当前状态并重新登记实例，不能逐表复制旧密文。

公开问题不得附带授权码、Client credential、管理员密码、数据库、生产 URL 或 Sunshine 凭据。
