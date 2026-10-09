# 05. 持久任务与恢复

配置读取、修改和重启都先成为 SQLite operation。`Idempotency-Key` 只能幂等同一 actor、设备、action 和
请求；同 key 不同内容返回冲突。请求在库中加密并绑定 operation ID/action AAD。

任务按设备串行。pending 等待在线 Client，running 已交付，succeeded/failed 有明确证据，unknown 表示
副作用结果无法证明。Server 重启或 WSS 在执行边界断开时不能自动重试副作用。

unknown 重连后以 inspect-only 询问 Client 的持久执行日志。Client 只能报告证据，不能执行命令。管理员
核对真实 Sunshine 后记录人工结论；resolve 不会重放原任务。
