# Sunshine Manager Server 当前功能与取舍清单

本文描述 `0.11.6` 与协议 `sunshine-management/2`。Server、Client 和协议只支持官方 Sunshine
`v2026.914.233613`，不注册旧协议、旧版本或旧状态的回退分支。

## 拓扑与所有权

```text
Browser ─HTTPS─> trusted ingress ─HTTP loopback─> Manager Server
Sunshine Client ─WSS/HTTPS─> trusted ingress ─HTTP loopback─> Manager Server
Sunshine Client ─HTTPS loopback（不校验证书身份）─> local Sunshine
Moonlight ───────────────────────────────────────────────────> Sunshine data plane
```

Server 拥有管理员会话、实例、授权码、Client credential 摘要、任务、观察和审计。Client 拥有 Sunshine
地址、凭据、本地身份和执行日志。完整 Sunshine 配置和密码不离开主机；视频与输入流不经过
Manager。

## 当前管理能力

| 领域 | 能力 | 边界 |
|---|---|---|
| 配置 | 统一定义提供类型、范围、Sunshine 版本、操作系统和前置条件；修改保留未受管字段 | 全配置修订检查减少本地编辑竞争，但 Sunshine 没有原子 CAS；网络监听、证书、凭据、任意路径和全局命令不开放 |
| 应用 | 列表、新建、编辑、删除、关闭当前应用、PNG 封面 | 使用列表修订和内容引用映射即时索引；命令字段只能通过专用应用结构提交，不存在通用 Shell |
| Moonlight | 提交 PIN、列出客户端、启用/禁用、单个或全部取消配对 | 与 Manager–Client 注册完全分离；禁用和取消配对会影响活动会话 |
| 日志与诊断 | Sunshine 日志按字节游标分页、客户端脱敏；读取 API、认证、版本、平台、配置修订和服务状态 | 单页上限 24 KiB；管理操作记录与 Sunshine 日志分开显示 |
| 显示与输入 | 重置显示设备持久状态、重置 Portal token、读取 VirtualHID/ViGEmBus 状态 | 使用固定上游端点，不接收路径、驱动安装命令或任意 HTTP |
| 服务 | 读取状态并启动、停止、重启 | Windows 固定 `SunshineService`，Linux 固定 systemd `sunshine.service`；macOS 未声明固定服务能力 |

Client 配对后直接启用 Sunshine 专用管理能力，不再询问重启、应用命令或服务控制权限。Web 对会中断会话、
删除资源或执行主机应用命令的单次操作仍要求管理员确认，并保留 15 分钟派发期限。

## 协议、任务与证据

Client 首帧必须上报精确协议、Client/Sunshine 版本、平台、全部受管字段和领域能力。旧 v1 消息、缺少字段的
能力对象、未知命令/结果字段和超过 64 KiB 的消息均失败关闭。

每条命令都有独立业务权限、资源命名空间、输入边界和对应结果。写入先持久化 Foundation operation，Client
再持久化副作用意图。断线、回执丢失或崩溃后只核对持久事实，不自动重做副作用。配置和应用修改可用修订/内容
核对；关闭应用、PIN 和维护等没有充分可观察证据的动作在丢回执时保持 `unknown`。

Sunshine 日志、应用列表和客户端列表都有数量/大小限制；报告必须与原命令类型相符。结果只描述已观察事实：
配置读回不证明编码器、显示或音频已经运行时生效，服务控制只有观察到目标状态才成功。

## 平台、数据与发行

- Server 发行目标为 Linux AMD64，监听固定为 loopback，外部 HTTPS/WSS 由可信入口终止。
- 管理 API 仅 `/api/v2`；SQLite Schema revision 7；正式发行绑定 Schema、Foundation revision、源码提交和全树摘要。
- 产品不内置 migration、backup、restore 或 key rotation；这些职责属于 `sarmg-upgrade` 的明确版本矩阵。
- 不提供任意 HTTP 代理、任意文件读写、远程 Shell、Sunshine 安装升级、视频转发、Server HA 或旧协议兼容。

## 验证矩阵

1. Rust fmt/check/clippy/test，协议命令/报告对应、权限、大小、版本、平台和字段边界。
2. 本地 HTTPS 夹具验证回环地址限制、无代理/重定向、应用修订、配对、日志脱敏、维护固定端点。
3. operation 幂等、安装代际、15 分钟写入期限、断线 unknown、只读核对和人工结论。
4. Chromium 与 Firefox 验证配置差异、应用稳定引用、Moonlight、日志、诊断、维护及服务控制。
5. 真实 Sunshine 与 Windows/Linux 固定服务适配器仍需在对应主机上验证实际副作用和恢复过程。
