# Sunshine Client 远端管理

实例列表为独立表格页面，功能导航位于顶部。新建实例只填写名称并取得该实例的长期授权码；Server 加密保存，设备状态页可查看和更换。更换后旧 Client credential 立即撤销，必须使用新码重新配对。
在 Sunshine 主机安装独立仓库的 [Client](https://github.com/isarmg/sunshine-manager-client)，由其主动建立 Manager WSS 通道。
Sunshine 用户名、密码和其自带 `cacert.pem` 仅在 Client 本机配置，不在 Manager 的连接表单填写。默认
自签名证书由 Client 精确固定；Manager 不接收证书或 Sunshine 凭据。

| 页面 | 当前功能 |
| --- | --- |
| 实例 | 每行一个设备，创建、选择、取消未使用的当前授权码 |
| 设备状态 | 查看/更换授权码，分别显示 Client 在线、Sunshine 可达、配置状态，永久撤销设备凭据 |
| Sunshine 配置 | 读取受管字段、编辑白名单字段、差异预览、保存、冲突提示、明确确认重启 |
| 任务记录 | 查询持久操作及结果，按现有规则核对不确定结果 |

当前仅开放 `read_config`、`patch_config` 和 `restart`。没有应用启动/准备命令编辑、
Moonlight PIN 管理、任意文件写入、日志透传、任意 HTTP 代理或自动下载安装入口。
Client 管理设备配对不是 Sunshine–Moonlight 串流配对，不改变原有串流链路。

## 配置与结果

配置修改携带预期修订、设置/删除字段及手动重启策略。Client 重新读取完整配置，
过滤 `status`、`platform`、`version` 等元数据，校验字段后合并保存，保留未修改字段。
未列入白名单的设置不能通过高级 JSON 绕过限制。完整配置修订也涵盖非受管字段，
发现修订冲突时先重新读取并确认差异。该检查不是 Sunshine 原生原子并发控制；受管字段应由 Manager 统一管理。

保存成功只表示配置文件已保存，页面显示等待重启。重启必须同时有 Client 本机授权和
管理员逐次确认，不自动打断串流。重启后重新核对服务与配置；无法证明运行时生效的设置仍显示待验证。

浏览器通过管理员 Session、CSRF 和 `/api/v2/sunshine/devices/{id}/tasks` 提交业务指令。
202 只表示任务已持久化，并不表示执行成功。任务复用 Foundation 的
`pending/running/succeeded/failed/unknown/dead_letter/resolved` 状态，重复投递先核对持久执行事实。
不确定结果不盲目重复重启，人工核对也不等于重新执行。刷新或退出不取消已接受任务。

## 验证边界

协议与固定上游版本依据见 [Client 管理协议](client-management-v1.md)，发行与实测范围见
[客户端候选记录](releases/client-0.1.0-rc.1.md)。

`web` 中的 `npm run test:browser` 覆盖管理界面；构建 Manager 和 Client 开发二进制后，
`node tests/client-end-to-end.mjs` 验证真实浏览器、Manager、独立 Client 与 HTTPS/WSS，
其中 Sunshine 仍是协议夹具，不可当作真实 Sunshine 硬件或运行时验收。
