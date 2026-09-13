# 08. 测试、调试与变更

按边界测试，而不只测成功路径：

1. protocol 的 unknown fields、大小、binding、permission、revision 和字段越界；
2. 实例授权码加密/查看/轮换、取消后删除、注册、撤销与事务审计；
3. Client ingress、WSS subprotocol/并发、Hello、心跳、session 替换和超时；
4. operation 幂等、串行、断线 unknown、inspect-only 和人工 resolve；
5. Web 实例列表、详情、日志、配置预览、授权码和直接语言切换；
6. Foundation revision 从 Cargo.lock 派生、release 全树校验和 workflow 权限。

变更协议时应先改 `protocol/`，随后 Server、独立 Client、Web validator、文档和 Release identity；当前
项目不保留旧字段或路由兼容。
