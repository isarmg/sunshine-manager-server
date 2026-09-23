# Sunshine Client 管理协议 v2

协议固定为 `sunshine-management/2`，WebSocket 子协议为 `sunshine-management.v2`，适配 Sunshine
`v2026.914.233613`。旧协议、旧 Sunshine 版本及缺少 v2 能力字段的 Hello 均被拒绝，不进行协商降级。

Client 主动建立 WSS 连接，Sunshine 凭据只保存在主机的受保护状态中。Manager 下发领域命令，Client 只调用
HTTPS 回环地址上的固定 Sunshine API 或固定系统服务适配器。协议没有任意 HTTP、Shell、文件路径或服务名入口。
Manager 拒绝无效设备凭据时，WebSocket 握手返回 401 与 `X-Sarmg-Error-Code: unauthorized`；入口代理须透传此响应头，Client 才能立即停止重连并等待重新配对。入口代理产生的无标识 401、入口校验 403 及临时服务错误按退避策略重连。

## 命令领域

- 配置：读取、按完整修订保存、明确重启。
- 应用：列表、按列表修订和内容引用新建/更新/删除、关闭当前应用、上传 30 KiB 以内 PNG 封面。
- Moonlight：提交短期 PIN、读取客户端、启停授权、单个/全部取消配对。
- 观察：分页脱敏 Sunshine 日志、API/认证/版本/平台/配置修订诊断、虚拟输入驱动状态。
- 维护：重置显示设备持久状态与 Portal token。
- 服务：读取状态，通过 Windows `SunshineService` 或 Linux `sunshine.service` 启动、停止、重启。

Client 通过 Hello 声明支持的 Sunshine 专用管理能力。macOS 没有经过定义的固定 Sunshine 服务，
因此只在该领域上报告不可用；其他 API 能力保持启用。

## 修订、去重与结果

配置修订覆盖完整本地配置；应用修订覆盖按 Sunshine 当前顺序规范化的完整应用数组。应用引用由单个应用内容派生，
执行更新或删除前重新读取列表，要求修订一致且引用唯一，然后才映射到 Sunshine 的瞬时数组索引。

Client 在副作用前持久化操作指纹和意图。同一 operation ID 只对应同一绑定、权限和命令；已有意图的重复投递只能
核对，不可重做。可核对的保存、删除、客户端状态和服务状态必须读回目标事实；关闭应用、PIN、封面和维护动作若
丢失回执则保持不确定。

每个报告必须与原命令类型匹配并通过大小、数量、修订、UUID 和文本边界。日志单页不超过 24 KiB，协议消息不超过
64 KiB。配置保存、重启接受、服务状态变化与运行时编码效果是不同证据，不互相冒充。

## 配置策略

`protocol/src/config.rs` 是唯一字段定义，向 Server Web 和 Client 校验同时提供类型、范围、支持版本、操作系统和
前置条件。编码器、输入、显示、音频与常用质量参数均按平台开放。网络监听、UPnP、Web 来源、证书、凭据、配置文件
路径、驱动安装及全局准备命令仍只能本机管理。

## 验证

协议策略测试覆盖旧 v1 拒绝、能力和确认、命令/报告对应、应用引用和消息边界。Client 的本地 HTTPS 测试使用真实
TLS 连接验证固定端点、证书拒绝、应用索引映射、配对、日志脱敏、维护和响应限制。浏览器验收在 Chromium 与 Firefox
覆盖所有 v2 领域。真实 Sunshine 与系统服务副作用仍以对应 Windows/Linux 主机验收为准。
