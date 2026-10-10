# xscs 功能范围

## 可以完成的工作

| 工作 | 使用入口 | 结果解释 |
|---|---|---|
| 管理多台 Sunshine 主机 | [实例管理](instance-management.md) | Client 在线与 Sunshine 可访问分别显示 |
| 编辑配置和应用 | [使用指南](usage.md) | 保存、重启与运行时生效分别确认 |
| Moonlight PIN 与已配对客户端 | [远端管理](remote-management.md) | Sunshine 本机 API 任务结果 |
| 查看日志、诊断与维护状态 | [使用指南](usage.md) | Sunshine 日志与 Manager 任务历史分开 |
| Sunshine 服务启停 | [协议参考](client-management-v1.md) | Windows / Ubuntu 固定服务，macOS 在本机处理 |

## 设计选择

- Server 集中存储持久任务，Client 主动出站 WSS，设备无需开放额外管理入口。
- Sunshine 凭据保存在 Client 主机，Server 只持有设备授权材料。
- 副作用前保存意图，结果不确定时由人工核对，降低重复执行风险。
- 受管配置字段由一份协议定义按平台和能力筛选；应用中的命令仍须由管理员审阅。

Sunshine 与 Moonlight 直接承载媒体流。当前服务管理配置、任务和观察，不承担串流转发或 Sunshine 安装。接口适配版本为 v2026.914.233613；硬件与串流表现需要目标设备验证。

## 如何核对实现

源码入口见[架构](architecture.md)，测试和构建见[开发指南](development.md)。单元测试、协议夹具、浏览器交互、原生服务和真实硬件是不同层级，记录结果时分别说明。
