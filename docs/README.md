# xscs 文档总览

本文档只描述 `1.0.0` 当前代码；`schema/generated/current_schema.sql`、`release.json`、Rust 合约与测试是
机器可执行事实源。

| 分类 | 文档 | 内容 |
|---|---|---|
| 初学者学习指南 | [beginner-guide/README.md](beginner-guide/README.md) | Sunshine、控制面、加密和异步操作入门 |
| 工作流程与流程树 | [project-workflow.md](project-workflow.md) | 启动、配对、Client 任务和发布流程 |
| 完整功能与取舍 | [feature-inventory-and-tradeoffs.md](feature-inventory-and-tradeoffs.md) | 功能边界、风险和架构选择 |
| 必要 README | [../README.md](../README.md) | 项目简介、功能、平台、快速部署和编译部署 |
| 运维 | [operations.md](operations.md) | 不可变部署、环境配置、doctor、故障与安全事件 |
| 当前发行说明 | [releases/1.0.0.md](releases/1.0.0.md) | 本版本功能和验证范围 |

工程约定与依赖来源见 [架构说明](architecture.md)，Rust unsafe 结论见 [审查记录](unsafe-audit.md)。

公共支撑的职责、单体依赖、平台边界与验证方法见[公共支撑说明](common-support.md)。

## 操作与开发入口

- [服务命令](cli.md)：显式初始化、运行、配置校验和状态查询
- [本地开发服务](local-service.md)：从源码启动管理 Web
- [开发与验证](development.md)：工具链、质量检查和运行边界
- [实例管理](instance-management.md)：管理设备与实例
- [客户端配对](simple-pairing.md)：建立管理端与代理的绑定
- [远端管理](remote-management.md)：任务、操作与验收
- [通信可靠性](communication-reliability.md)：执行结果、确认和故障恢复
- [客户端分平台部署](https://github.com/isarmg/xscc/blob/main/docs/platform-setup.md)：设备安装、配对、服务、诊断与卸载
