# Sunshine Manager Server 当前功能与取舍清单

本文只描述 `0.10.8` 当前实现。事实源依次为 `src/http.rs::router()`、`protocol/`、`schema/product.sql`、
Foundation 组合后的 Schema、Release identity 和管理 Web。拆分前由 Server 保存 Sunshine 密码并直接调用
Sunshine API 的架构已删除，不属于当前能力。

## 1. 当前拓扑与所有权

```text
Browser ─HTTPS─> trusted ingress ─HTTP loopback─> Manager Server
Sunshine Client ─WSS/HTTPS─> trusted ingress ─HTTP loopback─> Manager Server
Sunshine Client ─HTTPS loopback + pinned/system-trusted cert─> local Sunshine
Moonlight ───────────────────────────────────────────────────> Sunshine data plane
```

- Server 拥有管理员会话、实例、每实例授权码、Client credential 摘要、任务状态、配置快照和审计。
- Client 拥有 Sunshine 地址、用户名、密码、公开证书固定材料、受保护本地状态和执行日志。
- Server 从不接收 Sunshine 密码、证书私钥或完整本地配置，也不直接连接 Sunshine。
- 本产品不代理视频、不改变 Moonlight/Sunshine 串流链路，也不提供远程 Shell、应用管理、Moonlight
  客户端管理或封面代理。

## 2. 平台、身份与配置

| 能力 | 当前事实 | 关键边界 |
|---|---|---|
| Server 平台 | 仅 `x86_64-unknown-linux-gnu` | build、运行时和发行树三层拒绝其他目标 |
| 软件/API | 0.10.8，管理 API `/api/v2` | 不注册旧 API alias |
| 数据库 | Schema revision 7，SHA 由 release identity 固定 | 非当前 metadata/DDL 在业务写入前拒绝 |
| Foundation | Cargo.lock 中所有 Foundation crate 必须来自同一完整 revision | build.rs 自动从锁文件派生运行时 `foundation_revision`，不手写 |
| 监听 | `127.0.0.1:18104` | 非 loopback 配置直接拒绝，外部 TLS/WSS 由可信入口终止 |
| 静态 Web | 绝对路径、固定 layout、无链接别名 | 生产资源不能由服务账户拥有或被 group/world 写入 |
| Secret key | Base64 解码后恰 32 bytes，并有 key ID | 实例授权码与任务请求使用不同 AAD 域加密 |

`SUNSHINE_MANAGER_DATABASE_URL`、`STATIC_DIR`、`BIND`、`PRODUCTION`、`CREDENTIAL_KEY`、
`CREDENTIAL_KEY_ID` 与 bootstrap 管理员字段是当前配置面。旧 cover allowlist/proxy、Sunshine host/password
和产品级 Session TTL 变量均不受读取，不得继续出现在模板中。

## 3. 管理员与 Web

管理员认证、Argon2id、Session、CSRF、同源检查、统一错误和账户设置来自 Foundation。管理 Web 统一为：

- 实例列表：总览、每实例名称、注册/在线/Sunshine 状态和配置状态；
- 详细信息：实例长期授权码、名称、配对取消或终态删除、凭据撤销、当前快照、白名单配置预览与提交、
  明确授权的 Sunshine 重启；
- 日志：最近任务、终态、结果、核对证据，以及 unknown 的人工结论。

语言切换直接生效，不弹出“重新载入会丢失编辑”的确认框。页面状态是 Server 投影；SQLite 与 Client 回报
才是权威事实。

## 4. 实例授权与配对

| 能力 | 当前行为 | 失败关闭边界 |
|---|---|---|
| 创建实例 | 生成 64 位小写十六进制长期授权码 | 最多 4096 个实例；授权码以信封密文和独立摘要保存 |
| 查看授权码 | 管理员可从 Server 解密读取 | 响应 `no-store`，不写日志 |
| 更换授权码 | 新码替换密文/摘要，清除 installation、credential、session 与健康状态 | 旧 Client 必须重新配对 |
| 取消配对 | 首次取消清除 pending token | 再次删除已取消且从未注册的记录，解决残留无删除入口 |
| 注册 | Client 先解析授权码，再以 installation ID 和随机 credential 注册 | Manager/device/installation 三元绑定 |
| 撤销 | 已注册实例永久撤销 credential 和在线会话 | 重新使用需创建新实例 |

授权码不是一次性码，也不按时间自然过期；短期网络事务超时不改变其长期生命周期。

## 5. Client 通道

Client HTTP/WSS 入口是 `/sunshine-client/v1/pairing`、`/enroll`、`/identity` 和 `/connect`。它们只接受
来自 loopback TLS ingress 的 `X-Forwarded-Proto: https`，拒绝浏览器 Origin/Cookie；WSS 必须协商
`sunshine-management.v1`，消息最大 64 KiB，同时在线 Client 上限 256。

连接后 Client 首帧必须是精确 `Hello`，绑定 identity、协议、受支持 Sunshine 版本、平台、重启许可和
完整 managed field 集合。心跳回报 Sunshine 可达性与白名单配置快照。Server 45 秒未见活动、任务 110 秒
未回执、实例被撤销或会话被替换时断开连接。

## 6. 配置与任务协议

唯一指令为：

- `read_config`：读取当前白名单配置；
- `patch_config`：携带 expected revision、set/remove 和固定 manual restart policy；
- `restart`：携带 expected revision 且管理员与 Client 本地策略都明确允许。

当前白名单只有 `sunshine_name`、日志级别、QP、HEVC/AV1 模式、线程数、软件编码 preset 与有限 NVENC
字段。任意其他键、越界值、空变更、错误 revision、binding 或 permission 都被拒绝。Server 不接受完整
Sunshine JSON，不管理 apps/clients，也不把管理员输入转换成任意本机命令。

## 7. 持久任务与不确定性

管理写入先以 `Idempotency-Key` 持久化 Foundation operation，再通过 WSS 交给对应 Client；请求正文在库中
按 operation ID/action AAD 加密。每设备同一时刻只执行一个任务，不同设备可独立前进。

状态包括 pending、running、succeeded、failed、unknown、dead_letter、resolved。Server 或连接在执行边界
中断时不能推断 Sunshine 是否已产生副作用，因此恢复为 unknown；Client 随后只能以 inspect-only 回报本地
执行事实，不能重做副作用。管理员核对真实 Sunshine 状态后可记录 confirmed_succeeded、confirmed_failed
或 unable_to_confirm，resolve 不会再次执行任务。

## 8. 数据、发行与明确不提供

- SQLite 单实例运行；启动清除旧在线 session，并恢复运行中 operation 的不确定状态。
- 产品不内置 migration、backup、restore 或 key rotation。`sarmg-upgrade` 当前也未声明支持 0.10.8；
  不得使用 Sunshine 0.8.0 适配器处理当前库。
- 正式发行树包含 Server binary、Web、systemd、配置示例、README 和 manifest；文件、权限、大小、摘要、
  source revision、Schema 与 Foundation revision 必须形成同一不可变身份。
- 当前不提供多管理员角色、SSO、Server HA、非 Linux Server、自动 unknown 重试、任意配置字段、
  Sunshine 安装升级、视频转发、apps/clients/cover 路由或旧状态兼容。

## 9. 最低验证矩阵

1. Rust fmt/check/clippy/test 和 protocol unknown-field/大小/binding/字段边界。
2. 数据库 identity、授权码加密/查看/轮换、取消后删除、注册/撤销和审计事务。
3. WSS ingress、subprotocol、并发上限、Hello/heartbeat、会话替换、超时及 revoke。
4. operation 幂等、per-device 串行、断线 unknown、inspect-only 和人工 resolve。
5. React typecheck/build/browser：实例列表、详情、日志、授权码、取消/删除、配置与语言直接切换。
6. release identity、Foundation lock-derived revision、全树篡改拒绝和供应链权限门禁。
