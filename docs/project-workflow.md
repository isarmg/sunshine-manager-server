# Sunshine Manager 当前项目流程

## 1. 端到端链路

```text
管理员浏览器 -> HTTPS ingress -> Server 管理 API -> SQLite operation
                                                    -> WSS Client
                                                    -> 本机 Sunshine HTTPS
Client heartbeat/result -> WSS -> Server -> SQLite -> Web 列表/详情/日志
```

Server 只监听 loopback，不直接访问 Sunshine。Client 与 Sunshine 同机运行并拥有 Sunshine 凭据及公开证书
固定材料；Server 只保存 Client 实例身份、长期授权码、credential 摘要、配置投影和任务证据。

## 2. 实例生命周期

1. 管理员创建实例，Server 生成并加密保存 64 位十六进制长期授权码。
2. Client 用 Server origin 和授权码解析 manager/device identity，再提交 installation ID 与随机 credential。
3. Client 通过 WSS 发送绑定、能力和 Sunshine 版本，随后心跳回报可达性及白名单配置快照。
4. 管理员可查看或更换授权码；更换会清除旧绑定和 credential，Client 必须重新配对。
5. 未注册实例先取消 pending 配对，再执行一次删除；已注册实例只能永久撤销 credential。

## 3. 管理操作生命周期

管理写入需要 Session、CSRF、同源验证和 `Idempotency-Key`。Server 先验证当前协议并加密持久化 request，
再由 operation 管理器按设备领取。WSS 每设备只保留一个 pending 任务：read config、以 revision 为前置条件
的白名单 patch，或管理员和本地策略共同允许的 restart。

Client 执行前再次校验 binding、permission、revision、字段白名单和 operation 去重日志。连接中断导致执行
结果不可证明时进入 unknown；重连后的 inspect-only 只能读取证据，不能重复副作用。管理员核对本机实际
状态后记录人工 resolution。

## 4. 开发顺序

1. 先修改 `protocol/` 的严格类型、校验和协议测试。
2. 修改 `src/http.rs` 路由及 `src/db.rs`/`src/operations.rs` 的事务和恢复语义。
3. 同步独立 Client，并验证真实 Sunshine 的证书、配置文件和重启边界。
4. 修改 Web 的实例列表、详情、日志及 API runtime validator。
5. 同步 Schema/release identity、配置模板、文档和供应链门禁。
6. 运行完整 CI 后才创建 annotated tag；产品不保留旧 route/field/schema fallback。

## 5. 发布边界

Server 仅发布 Linux AMD64 binary 与 Web。build.rs 从 `Cargo.lock` 自动派生唯一 Foundation revision；正式
binary 还绑定源码 revision、Schema revision 7 和 Web 资产。Release 树需通过自校验和篡改负例。

产品没有 backup/restore/migration/key rotation。当前 `sarmg-upgrade` 未支持 0.11.1，不能使用旧 0.8.1
适配器；需要转换时先在升级仓增加精确输入/输出身份和隔离恢复验收。
