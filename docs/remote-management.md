# Sunshine Client 远端管理

实例列表为独立表格页面，功能导航位于顶部。新建实例只填写名称并取得该实例的长期授权码；Server 加密保存，设备状态页可查看和更换。更换后旧 Client credential 立即撤销，必须使用新码重新配对。
在 Sunshine 主机安装独立仓库的 [Client](https://github.com/isarmg/sunshine-manager-client)，由其主动建立 Manager WSS 通道。
Sunshine 用户名和密码仅在 Client 本机配置，不在 Manager 的连接表单填写。Client 只接受 HTTPS 回环 IP
地址，且不校验这条本机连接的证书身份；Manager 不接收 Sunshine 凭据。

| 页面 | 当前功能 |
| --- | --- |
| 实例列表 | 按操作系统显示实例总数/在线统计，再按行显示实例简要状态并可选择实例 |
| 详细信息 | 授权码与实例、配置、应用/封面、Moonlight 配对、Sunshine 日志/诊断、显示/输入维护和固定服务控制 |
| 日志 | 查询 Manager 持久操作及结果，按现有规则核对不确定结果；与 Sunshine 本机日志明确分开 |

## 配置字段合同

当前受管字段由 `sunshine-client-protocol::config::FIELD_DEFINITIONS` 唯一定义。每项同时声明类型、
数值范围或枚举、最大长度、是否需要重启、已验证的 Sunshine 版本、适用操作系统及硬件前置条件。
协议校验、握手字段集合和管理页都消费这份定义；管理页通过受保护的
`GET /api/v2/sunshine/config-fields` 获取定义，并与 Client 上报的 `managed_fields`、Sunshine 版本和
操作系统取交集。TypeScript 只保留翻译标签，不再复制范围和选项。

新增字段必须先进入该定义并完成对应 Client 版本的真实 Sunshine 验收。字段元数据出现在 Server
源码中不表示旧 Client 自动获得能力；协议包仍按发布提交固定，Server 与 Client 发布时必须更新同一
协议提交并运行完整链路测试。

协议 v2 还开放专用的应用、Moonlight、日志/诊断、维护和服务命令。应用启动、准备和分离命令作为应用
资源字段明确显示执行风险；没有通用 Shell、任意文件写入、任意 HTTP 代理或自动下载安装入口。
Client 管理设备配对不是 Sunshine–Moonlight 串流配对，不改变原有串流链路。

## 配置与结果

配置修改携带预期修订、设置/删除字段及手动重启策略。Client 重新读取完整配置，
过滤 `status`、`platform`、`version` 等元数据，校验字段后合并保存，保留未修改字段。
未列入白名单的设置不能通过高级 JSON 绕过限制。完整配置修订也涵盖非受管字段，
发现修订冲突时先重新读取并确认差异。该检查不是 Sunshine 原生原子并发控制；受管字段应由 Manager 统一管理。

保存成功只表示配置文件已保存，页面显示等待重启。Client 配对后直接启用 Sunshine 专用能力；可能中断
串流、删除资源或执行主机命令的单次操作仍需管理员确认。重启后重新核对服务与配置；无法证明运行时生效的
设置仍显示待验证。所有写操作自创建起只在 15 分钟内允许派发；设备在期限后才上线时，Server 将操作标记为
执行期限已过，管理员需重新查看当前资源并提交。只读操作不套用该短期限。

浏览器通过管理员 Session、CSRF 和 `/api/v2/sunshine/devices/{id}/tasks` 提交业务指令。
202 只表示任务已持久化，并不表示执行成功。任务复用 Foundation 的
`pending/running/succeeded/failed/unknown/dead_letter/resolved` 状态，重复投递先核对持久执行事实。
不确定结果不盲目重复重启，人工核对也不等于重新执行。刷新或退出不取消已接受任务。

## 验证边界

协议与固定上游版本依据见 [Client 管理协议](client-management-v2.md)，发行与实测范围见
[客户端候选记录](releases/client-0.1.0-rc.1.md)。

`web` 中的 `npm run test:browser` 覆盖管理界面；构建 Manager 和 Client 开发二进制后，
`node tests/client-end-to-end.mjs` 验证真实浏览器、Manager、独立 Client 与 HTTPS/WSS，
其中 Sunshine 仍是协议夹具，不可当作真实 Sunshine 硬件或运行时验收。
