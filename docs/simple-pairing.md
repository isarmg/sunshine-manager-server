# 简化配对（Server 0.11.3 / Client 0.2.0）

客户端 UI 仅填写 Server 地址、256-bit 实例授权码、本机 Sunshine 回环地址/端口及账号密码；使用 Sunshine
自带证书时再选择其配置项 `cert` 对应的 `cacert.pem`。Manager ID、设备 ID 不再是用户输入项。客户端与
Server 需同时部署支持本流程的版本。

`POST /sunshine-client/v2/pairing` 接受唯一字段 `authorization_code`，返回 `manager_id`、`device_id`。
沿用 Client 的可信 HTTPS 入口限制、无 Cookie/Origin、16 KiB 请求上限和 `Cache-Control: no-store`。
查询不消费授权码；仅匹配待配对、未撤销且未取消的实例摘要。错误授权码统一拒绝，不回显秘密。
真正注册仍通过 `/sunshine-client/v2/enroll` 原子绑定独立设备凭据；授权码不会被删除。查询后发生取消、撤销或授权码轮换会使注册失败。
客户端在注册前持久化独立随机凭据，回执丢失仍使用原有 identity 查询恢复，不创建另一套任务状态机。Server 更换授权码会撤销当前凭据；Client 先用新码验证仍为原实例，再通过显式 `pair replace` 重新绑定。

Manager HTTPS/WSS 始终使用系统信任库并验证名称；Windows LocalSystem 服务使用计算机信任上下文。
本机 Sunshine 若提供 `sunshine_certificate_path`（或受保护输入中的 PEM），Client 将证书 DER 精确固定，
从而支持 Sunshine 默认无回环 IP SAN 的自签名证书。TLS 握手还会验证服务端持有对应私钥；证书不一致时
会在发送 Sunshine Basic 凭据前失败。实现不关闭 TLS、不信任其他自签名证书、不跟随重定向，也不把凭据
或证书写入日志。Sunshine 证书更新后，管理员需同时更换实例授权码，再由 Client 携带新证书和新授权码
执行显式 `pair replace`。

Windows Client 最低 Windows 11 x86_64/build 22000。正常菜单退出会先停止服务，停止失败则保留托盘；服务不再无条件随开机自动运行。
Sunshine 重启与退出 Client 是两个操作；Client 仍独立于 Sunshine 进程，不修改视频串流链路。
