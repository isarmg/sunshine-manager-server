# xscs 运维文档

## 1. 生产布局

```text
/opt/isarmg/xscs/releases/1.0.0/  root-owned read-only release
/etc/isarmg/xscs.env             0600 environment
/var/lib/isarmg/xscs/db/xscs.sqlite3  SQLite
/run/isarmg/xscs/                locks/runtime
```

systemd 使用 `xscs`，直接执行：

```text
/opt/isarmg/xscs/releases/1.0.0/bin/xscs \
  run --release-root /opt/isarmg/xscs/releases/1.0.0
```

安装目录使用唯一受控的 `current` 指针；`run` 按当前版本实体路径解析该指针，发行树内部仍拒绝 symlink、特殊文件、hardlink asset、服务账户拥有的
asset 和 group/world writable 内容。

## 2. 构建与安装发行物

从干净且 annotated `v1.0.0` 精确指向 HEAD 的 checkout：

```bash
python3 scripts/package-release.py /absolute/release-output
```

当前 Server Rust 固定 xcss 1.0.0 / `627d988a4ed471469ed4fdce8af0ea6b5c131ce6`；一个 @xcss/web 包
使用同版正式 Release tarball 和 lockfile integrity，不依赖相邻 xcss checkout。独立 CI 已通过，
见[本项目当前 CI](https://github.com/isarmg/xscs/actions)与[正式发行资产](https://github.com/isarmg/xscs/releases)。
这证明当前源码的独立依赖与构建，不表示现有产品 tag 已包含随后主分支的改动；正式交付仍须使用
与产品版本、完整源码提交一致且通过全部门禁的发行树，不得覆盖旧 tag 或资产。
xcss 不是生产运行服务，发行树中不增加其 daemon、配置或 socket。

输出已存在时拒绝覆盖。安装：

```bash
tar -xzf xscs-1.0.0-x86_64-unknown-linux-gnu.tar.gz \
  -C /opt/isarmg/xscs/releases
/opt/isarmg/xscs/releases/1.0.0/bin/xscs \
  verify-release --root /opt/isarmg/xscs/releases/1.0.0
```

安装仓库内 systemd unit，创建专用账户、私有状态目录和 `/etc/isarmg/xscs.env`。先以服务账户和同一环境执行显式 `init`，成功后再 enable/start；普通 `run` 不创建数据库或管理员。
同版本不得合并或覆盖。

正式归档只生成 `x86_64-unknown-linux-gnu` Server；管理 Web 及字体内嵌在二进制中，`web-assets.json` 记录编译资源清单并与二进制精确绑定。该限制不要求受管 Sunshine Host 或
Moonlight 客户端使用 AMD64，也不改变 Sunshine 上游 API；它们仍是本控制面的外部端。

## 3. 环境配置

| 变量 | 默认/要求 | 说明 |
|---|---|---|
| `XSCS_DATABASE_URL` | 必填 | 当前 SQLite URL |
| `..._BIND` | `127.0.0.1:18104` | 生产回环监听 |
| `XCSS_DEV_WEB_DIR` | 开发可选，生产禁止 | 仅 unbound 构建且 `PRODUCTION=false` 时可指定绝对目录；通过 xcss 每次读取当前文件 |
| `..._CREDENTIAL_KEY_ID` | 当前 key ID | 必须与当前库中全部 envelope 一致 |
| `..._CREDENTIAL_KEY` | Base64 32 字节 | 独立秘密管理，禁止日志/仓库 |
| `..._BOOTSTRAP_ADMIN_USERNAME` | `admin` | 仅显式 `init` 创建首个管理员；1–64 字节 printable ASCII candidate，解析后必须得到 3–64 字节 canonical username |
| `..._BOOTSTRAP_ADMIN_PASSWORD` | 仅显式 `init` 必填 | 12–1024 字节且无 ASCII control；创建后从长期环境文件移除明文并轮换初始密码 |

Session、Cookie、CSRF 和登录限流使用 xcss `AdministratorPolicyV1` 固定值，产品环境变量不能覆盖。
Server 不保存 Sunshine 管理密码；该密码仅由 Client 在 Sunshine 主机本地受保护保存。

## 4. 管理命令

```bash
xscs identity
xscs verify-release --root /opt/isarmg/xscs/releases/1.0.0
xscs init --data-dir /absolute/new-private-data
xscs config validate --data-dir /absolute/current-private-data --json
xscs status --data-dir /absolute/current-private-data --json
xscs doctor
printf '%s\n' '<new-secret>' | xscs admin-reset-password \
  --database-url sqlite:///path/app.db --username admin
```

`init` 只接受全新私有空目录，使用当前配置中的 bootstrap username/password 创建数据库和首个管理员，拒绝覆盖已有状态。配置中的 SQLite 文件须是该数据目录的直接子文件。`doctor` 和管理员写命令需要 maintenance 排他锁；先停服务。避免把真实密码留在 Shell history，使用受控 Secret
注入或临时受保护终端。`admin-reset-password` 从标准输入读取一行密码，避免密码进入进程参数，
并只接受 canonical 化后精确匹配的当前 username。

管理员身份不是邮箱。登录候选必须为 1–64 个可打印 ASCII 字节；Server 只执行 `trim_ascii()` 和
ASCII 小写化，然后要求持久值为 3–64 字节、首尾是字母/数字且字符只来自 `[a-z0-9._-]`。`@`、Unicode、
控制字符和其他符号均拒绝；数据库 CHECK、启动存量检查、Session DTO、账户限流键和 Web 表单使用同一
username。不存在 `EMAIL` 环境变量、`--email` 参数、JSON `email` 字段或兼容别名。

## 5. 诊断命令 doctor

`doctor` 验证 product metadata、现场 Schema fingerprint、SQLite integrity/foreign keys、可回滚写事务，
并验证 Manager 身份及全部持久 operation request。请求必须通过当前任务解码和密文身份校验；
不会尝试旧格式或不带 AAD 的 fallback，也不会连接 Sunshine。它不保留
业务探针行；它不是纯只读命令，因为会执行随后回滚的写事务。它取得数据目录与数据库的排他维护锁，不能与运行中的 Server 并行；需要只读检查时使用 `config validate --json`。

其中 WAL/FULL synchronous/foreign-key/busy-timeout 连接基线、checkpoint、integrity/FK 和 Schema 指纹算法来自
xcss；数据库文件权限、main/WAL/journal 私有代际快照预检、DDL/init、失败清理、运行/maintenance
锁仍由 xscs 负责。预检只读取源 `-shm` 的类型与身份，不在源库上建立 SQLite 连接；因此非当前
库拒绝不会改写 SHM 锁字节。故障定位时不要绕过任一层，也不要用 xcss API 现场创建或转换非当前库。

若 Schema 不符，不得手改 metadata；若解密失败，先确认 key ID、key 文件来源和权限，不得添加“尝试
其他 key”、空 AAD 或忽略记录身份的 fallback。即使 envelope 仍以 `xscs:sgev1:` 开头，也不能据此前缀
判定它属于当前合同；AES-GCM tag 必须在当前确定性 AAD 下验证通过。
同一 master key 除直接供 AES 使用外，还派生两把 HMAC key，但不会暴露通用 HMAC key API：request fingerprint
和 Idempotency-Key hash 各有固定且不同的 HKDF info。换 master key 会同时改变密文可用性和两个 HMAC 域，
不得手改 BLOB、回退裸 SHA-256 或尝试另一域的 key。

## 6. 反向代理和网络

公网 TLS proxy 保留原 Host，并确保 Session Cookie Secure；同时由 proxy 设置经验证的 HSTS/CSP 等浏览器
响应策略，Server 当前不终止浏览器 TLS 或注入这些 header。应用端口只对可信本机 proxy 回环开放。
proxy 必须覆盖而非信任外部传入的 `X-Forwarded-Proto`，仅在已验证的 TLS 连接上设置为 `https`，
并支持 `/xscc/v1/connect` 的 WebSocket Upgrade。独立设备通道拒绝 Cookie/Origin。

Client 主动连接 Server 的 WSS，无需在 Sunshine 主机开放额外入站管理端口。
只有 Client 通过本机回环 HTTPS 访问 Sunshine；Server 不直接连接 Sunshine。Manager HTTPS/WSS 仅接受
系统已信任且名称匹配的证书；私有 CA 必须先安全安装到 Client 实际运行身份使用的系统信任库，Windows
LocalSystem 使用计算机信任存储。本机 Sunshine 必须使用 HTTPS 回环 IP 字面量；Client 不校验这条本机
连接的证书身份，以支持默认无回环 IP SAN 的自签名证书。它不使用代理、不跟随重定向，也不接受非回环
地址、URL 凭据、查询、片段或额外路径。详见
[简化配对](simple-pairing.md)。

## 7. 当前数据与凭据

当前数据库身份是 xscs `1.0.0`、Schema revision 1、SHA-256
`b3fdff2217ea2ba4a384e3ade29cbf87a949d63392bdff3895916ff2377bd1ba`，软件版本独立由发行 identity 表达。
`run` 只接受完整当前状态；不能手改 metadata 或拼接数据库与密钥。Secret 泄露时隔离服务并撤销管理员会话
和实例凭据。主 key 泄露时建立全新当前状态并重新登记实例，不能逐表复制旧密文。

## 8. 监控与故障定位

1. 检查 systemd、release verify 和 Web asset 是否通过。
2. 检查反向代理 TLS、Cookie、Origin/Host 与系统时钟。
3. 运行 doctor，确认 SQLite、写能力和全部密文。
4. 对 pending 查看 Client 在线状态、WSS 入口和任务队列；对 unknown 先查 Sunshine 实际状态，禁止盲重试。
5. 分别核验 Client 在线、Sunshine 可达和配置状态；“已保存”“待重启”“待验证”不等于运行时已生效。
6. 监控 SQLite/WAL、operation backlog、unknown 数量、磁盘和 inode。

API 错误必须同时检查 HTTP status 与稳定 `code`；`message` 只用于展示。若 Web 报
`invalid_error_response`，先检查代理是否改写 JSON/content-type 或服务端是否返回非当前错误形状；若为
`invalid_response_shape`，说明 2xx 正文已偏离当前端点合同，不应在浏览器添加宽松分支。
xcss Runtime 校验传入的 `x-request-id`，缺失时生成并写入响应头及错误 envelope；Web 不提供诊断页面。
共享 Web Shell 只显示安全提示和校验后的关联 ID，不显示内部异常文本。

持久操作每次领取使用唯一 owner；执行最多 90 秒，租约 120 秒。完成写入要求当前未过期 owner；
远端效果可能发生而提交失败时，仅将捕获的 claim 标为 Unknown，不重复执行。退出停止领取并有界等待，
中止的 Running 由独占启动恢复标为 Unknown。人工处理支持 confirmed_succeeded、confirmed_failed 和
unable_to_confirm；最后一项仅 Unknown → DeadLetter，所有决定均不重放原操作。
人工处理与操作者审计、审计物化与 outbox 确認分别在同一事务中提交。

## 9. 安全事件

隔离公网和受影响 Client，保全 release SHA、数据库 generation、审计和日志，撤销管理员 Session，
轮换管理员密码、实例授权码、Client credential 与入口 TLS key；Sunshine 本机凭据只在对应 Client 主机上
轮换。credential key 一旦确认泄露，应停用该数据库，建立
全新当前数据库/key 并重新登记实例，不能继续运行或自行批量改密文。使用 GitHub Private Vulnerability
Reporting；公开 issue 不得包含生产实例、数据库、密文、key、URL 或请求正文。只支持
当前发布版本与当前 `main`。

## 统一 Web 构建

运行 `npm --prefix web ci` 后，使用 `web/node_modules/.bin/xcss-build-server --config xcss-web-build.json --mode development --no-install`，统一完成 Web、Rust 与实际二进制资源清单验收。正式发行脚本使用相同入口的 release 模式，固定完整源码 revision，并在原有不可变发行树中校验部署身份。更新外部清单及其摘要不能替换已内嵌的浏览器资源。

初次安装须先验证 `/opt/isarmg/xscs/releases/1.0.0`，再在同一所有者、实体 `0755` 安装目录与 `releases` 目录下创建绝对链接 `current` 指向该版本。服务执行 `current/bin/xscs run --release-root /opt/isarmg/xscs/current`。首个管理员与数据库仍须通过 `init` 显式创建。

## 任务历史容量

任务日志每次读取最多 50 条，首页、上一页和下一页使用稳定游标；Web 只保留当前页。每实例最多一个历史读取，全服务最多四个，查询的 SQLite 执行预算为 3 秒，HTTP 预算为 5 秒，响应最多 8 MiB。

新任务写入同样每实例最多一个、全服务最多四个非等待准入，容量检查有 3 秒 SQLite 执行预算。历史与不确定结果不自动删除。新任务的默认准入上限是每实例 100,000 条或 1 GiB 的载荷、WAL、回执与审计预算、全服务 1,000,000 条，以及 SQLite 页、WAL 和未来回执、配置快照、审计预留合计 8 GiB；磁盘另须保留 1 GiB。预留按未写报告和保留行计费，因此字节上限可能先于行数上限达到。满额返回 `history_capacity_exhausted` / 503，任务未被接受；已有记录仍可查询，幂等重试仍读取原记录，已接受任务仍可提交最终结果。不同应用共享磁盘时，其他写入仍可能消耗余量，持久化失败会被明确报告。

Client 连接全服务最多 256 个、每实例最多一个；重复连接返回可重试 429，释放原连接后可以重新连接，避免单实例占满其他实例的连接名额。

## 当前中立接口与旧版数据处理

当前版本只使用 `.state-instance.lock`、`.state-maintenance.lock`、`.state-maintenance-pending.json` 和 `.state-atomic-` 临时文件前缀；离线升级工具采用 `.release-upgrade` 工作目录。服务身份头为 `x-service`，错误码头为 `x-error-code`，健康状态中的公共源码修订字段为 `common_revision`。管理会话采用 `__Host-admin-xscs-session`，显式开发模式采用 `admin-xscs-session`；生产 Cookie 的 Secure、HttpOnly、SameSite、Path 和 CSRF 约束继续生效。资源清单格式为 `web-assets-v1`，公共数据库内部表及索引采用 `_common_` 前缀。

这些接口没有旧名称别名或旧版兼容分支。旧版升级前，先按本文的停服步骤停止服务、配套客户端及全部维护工具；确认全部进程退出后，完整备份配置、SQLite 数据库及其 WAL/SHM、业务文件和必要的私有凭据。备份包含敏感数据，应保留原有访问权限并离线保存。

保留旧数据目录，按当前安装步骤配置新的私有数据目录，执行显式 `init` 初始化，随后运行 `config validate`，再启动服务、登录管理页面并重新配对客户端。旧配置应人工审阅后填写当前字段，不能整体覆盖新目录。旧业务数据需要另行处理；当前版本不提供自动迁移。不得让旧、新版本同时写同一目录，不得通过删锁文件或修改数据库 metadata 强制启动；当前结构指纹包含实际表名、索引名和 SQL，仅改名称不能证明数据符合当前合同。
