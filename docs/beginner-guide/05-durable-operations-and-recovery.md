# 05. 理解持久化与断线

Client 在副作用前保存意图，执行后持久结果；Server 收存后确认。WSS 断开不取消已开始的执行。unknown 表示效果未确认。

读[通信可靠性](../communication-reliability.md)，用协议夹具分别测试接收前断线、执行后回执丢失。

[学习路线](README.md) · [文档首页](../README.md)
