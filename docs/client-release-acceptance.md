# Client 验证范围

当前 Client 为独立 [xscc 仓库](https://github.com/isarmg/xscc)，适配 Sunshine v2026.914.233613。使用当前提交的 CI 和制品 manifest 核对源码；历史候选版的实测不代表当前版本结果。

## 分层验证

| 层级 | 验证内容 | 证据入口 |
|---|---|---|
| 协议与内核 | 命令、结果、去重、容量与失败恢复 | 两仓 Rust 测试 |
| 浏览器 | 配置、应用、配对和结果显示 | xscs Web 浏览器测试 |
| HTTPS/WSS | 注册、连接、回执丢失与重连 | Client HTTPS 测试与管理端端到端夹具 |
| 原生安装 | 平台包、账户、权限、服务启停与卸载 | xscc 各平台 CI |
| 真实 Sunshine | 配置读写、实际服务与平台 API | 目标主机独立测试记录 |
| Moonlight 与硬件 | 配对、编码和串流体验 | 实际 Sunshine/Moonlight 设备验证 |

## 在一台目标主机确认安装

1. 核对原生包摘要、版本及平台。
2. 完成 setup，确认 Client 服务运行及管理台在线。
3. 读取 Sunshine 配置或状态，确认往返任务有结果。
4. 需要验证写入时，先选择明确的测试资源和可中断时间，再保存、读回并验证效果。
5. 记录平台、Sunshine 版本、软件源码身份、命令和结果。

安装包签名状态见 [xscc 安装说明](https://github.com/isarmg/xscc/blob/main/docs/platform-setup.md)。真实设备范围随各次记录说明，不由本页推定全部平台或硬件已经验收。
