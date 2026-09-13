# 07. Client 协议与当前合同

Client HTTP 入口完成授权码解析、注册和 identity 检查；WSS `/sunshine-client/v1/connect` 承载 Hello、
heartbeat、task、result 与 revoke。消息上限 64 KiB，协议和 subprotocol 都是精确常量。

指令仅有 read config、patch config 和 restart。patch 必须提供 expected revision，只能 set/remove 当前
白名单字段；restart 同时要求管理员确认与 Client 本地 `restart_allowed`。Client 直接读取受保护 Sunshine
配置文件并通过本机 HTTPS 检查效果，Server 不接受整个配置文件。

Sunshine 自带自签名证书可由 Client 精确固定公开 PEM；未固定时使用系统信任和正常名称验证。任何模式
都没有 insecure 开关，私钥永远不发送给 Server。
