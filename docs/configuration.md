# 配置 xscs

首次安装使用[部署手册](operations.md)。生产配置位于 `/etc/isarmg/xscs.env`，由 systemd 的 EnvironmentFile 加载。修改后重启 xscs 并检查 `/readyz`。

## 环境变量

| 变量 | 默认或要求 | 用途 |
|---|---|---|
| `XSCS_DATABASE_URL` | SQLite URL | 默认安装使用 `sqlite:///var/lib/isarmg/xscs/db/xscs.sqlite3` |
| `XSCS_DATA_DIR` | 数据目录绝对路径 | 可据此派生数据库路径；数据库须是其直接子文件 |
| `XSCS_BIND` | `127.0.0.1:18104` | HTTP 后端监听地址 |
| `XSCS_PRODUCTION` | `true` | 生产 Session/Cookie 和发行资源策略 |
| `XSCS_CREDENTIAL_KEY` | 标准 Base64 的 32 个随机字节 | 加密持久授权数据，并派生任务指纹使用的密钥 |
| `XSCS_CREDENTIAL_KEY_ID` | 与数据库密文相符的 ID | 默认安装设为 `primary` |
| `XSCS_BOOTSTRAP_ADMIN_USERNAME` | `admin` | 首次 init 创建的用户名 |
| `XSCS_BOOTSTRAP_ADMIN_PASSWORD` | 首次 init 必填，12–1024 字节，无 ASCII 控制字符 | 创建后移除长期文件里的明文 |
| `XCSS_DEV_WEB_DIR` | 开发可选绝对目录 | 未绑定源码的开发构建且 production=false 时使用 |

保留 credential key 及 ID 与数据库配套；日常更换管理员密码或实例授权码时保持它们不变。更换主密钥会影响已有密文及任务身份验证。

## 管理员与连接

用户名使用 3–64 个 ASCII 字符，首尾是字母或数字，中间可用 `._-`。输入会去除首尾 ASCII 空白并转小写。管理角色固定为 admin，用户名可以修改。密码与账号维护见[账号设置](account-settings.md)。

浏览器连接使用 HTTPS 代理及管理员 Session；Client 使用 HTTPS/WSS 和独立设备凭据。Sunshine 用户名、密码保存在各自的 Client 主机。Cookie、CSRF、会话时间和登录限流由公共认证策略固定。

## 验证修改

以服务账户加载同一环境执行 `config validate --json`，核对来源、当前结构身份和状态路径，完整命令见[运维](administration.md)。命令行、环境、JSON 文件及默认值的优先级见[命令参考](cli.md)。未知环境变量不会自动变成有效参数，按本表核对名称。

本机开发使用[本地服务工具](local-service.md)，它生成独立配置和随机凭据。
