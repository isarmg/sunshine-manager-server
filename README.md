# xscs

当前工作树为 `1.0.0` 发行候选；正式源码、标签与资产以通过 CI 的精准 Source 和 Release manifest 为准。

当前启动入口和初始化边界见 [服务命令](docs/cli.md)。部署须先显式 `init`，再 `run`；配置验证和状态查询失败会返回非零退出码。

xscs `1.0.0` 是集中管理多台 Sunshine 主机的自托管服务。Rust/Axum Server 负责管理员、设备、任务和审计，内置 React Web 提供配置、应用、Moonlight PIN、日志、诊断、维护和 Sunshine 服务控制。

实际操作由每台主机上的独立 [xscc](https://github.com/isarmg/xscc) 完成；Manager 不保存 Sunshine 管理密码，也不代理 Sunshine–Moonlight 媒体流。正式 Server 仅支持 Linux AMD64 GNU（`x86_64-unknown-linux-gnu`）。

设备侧从安装、配对/重新配对到服务或后台任务管理、诊断与卸载，见独立 [Client 分平台部署指南](https://github.com/isarmg/xscc/blob/main/docs/platform-setup.md)。

## 配置概览

从模板创建生产环境文件，并生成恰好 32 bytes 的凭据加密密钥：

```sh
sudo install -d -m 0750 /etc/isarmg
sudo install -m 0600 config/xscs.env.example \
  /etc/isarmg/xscs.env
openssl rand -base64 32
sudoedit /etc/isarmg/xscs.env
```

至少替换 `XSCS_CREDENTIAL_KEY` 和管理员密码，并检查数据库、静态资源与监听地址。发行包中的服务启动命令为：

```sh
/opt/isarmg/xscs/current/bin/xscs \
  run --release-root /opt/isarmg/xscs/current
```

建议只监听 loopback，由 HTTPS 反向代理提供浏览器入口。部署、账号维护、Client 配对和远端管理见运维文档。

## 开发验证

```sh
npm --prefix web ci
web/node_modules/.bin/xcss-build-server --config xcss-web-build.json --mode development --no-install
python3 scripts/check-workflow-supply-chain.py
cargo +1.99.0 fmt --all -- --check
cargo +1.99.0 clippy --locked --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
cargo +1.99.0 test --locked --target x86_64-unknown-linux-gnu
```

## 文档

- [文档总览](docs/README.md)
- [初学者指南](docs/beginner-guide/README.md)
- [实例管理](docs/instance-management.md)
- [Client 配对](docs/simple-pairing.md)
- [远端管理](docs/remote-management.md)
- [部署与运维](docs/operations.md)

代码采用 [Apache License 2.0](LICENSE-APACHE)。

当前发布版本：**1.0.0**。参见 [1.0.0 发布说明](docs/releases/1.0.0.md)。

公共支撑的职责、单体依赖、平台边界与验证方法见[公共支撑说明](docs/common-support.md)。
