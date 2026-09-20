# 07. Client 协议与当前合同

Client HTTP 入口完成授权码解析、注册和 identity 检查；WSS `/sunshine-client/v2/connect` 承载 Hello、
heartbeat、task、result 与 revoke。消息上限 64 KiB，协议和 subprotocol 都是精确常量。

协议包含配置、应用、Moonlight 配对、日志/诊断、维护和固定服务控制等专用命令，不含通用 Shell、任意路径
或任意 HTTP。配置 patch 必须提供 expected revision，只能 set/remove 当前白名单字段；restart 同时要求
管理员确认且 Client 能力声明 `restart_allowed`。Client 直接读取受保护 Sunshine 配置并通过本机 HTTPS
检查效果，Server 不接收整个配置文件。

Manager HTTPS/WSS 使用系统信任并验证名称。本机 Sunshine 地址必须是 HTTPS 回环 IP 字面量，Client 不校验
该本机连接的证书身份；请求不使用代理、不跟随重定向，并对每个请求使用 Sunshine 凭据认证。凭据和私钥
永远不发送给 Server。
