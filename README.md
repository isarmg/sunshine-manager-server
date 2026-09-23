# Sunshine Manager

Sunshine Manager `0.11.11` 是集中管理多台 Sunshine 主机的自托管服务。Rust/Axum Server 负责管理员、设备、任务和审计，内置 React Web 提供配置、应用、Moonlight PIN、日志、诊断、维护和 Sunshine 服务控制。

实际操作由每台主机上的独立 [sunshine-manager-client](https://github.com/isarmg/sunshine-manager-client) 完成；Manager 不保存 Sunshine 管理密码，也不代理 Sunshine–Moonlight 媒体流。正式 Server 仅支持 Linux AMD64 GNU（`x86_64-unknown-linux-gnu`）。

## 配置概览

从模板创建生产环境文件，并生成恰好 32 bytes 的凭据加密密钥：

```sh
sudo install -d -m 0750 /etc/isarmg
sudo install -m 0600 config/sunshine-manager.env.example \
  /etc/isarmg/sunshine-manager.env
openssl rand -base64 32
sudoedit /etc/isarmg/sunshine-manager.env
```

至少替换 `SUNSHINE_MANAGER_CREDENTIAL_KEY` 和管理员密码，并检查数据库、静态资源与监听地址。发行包中的服务启动命令为：

```sh
/opt/isarmg/sunshine-manager/releases/0.11.11/bin/sunshine-manager \
  serve-release --root /opt/isarmg/sunshine-manager/releases/0.11.11
```

建议只监听 loopback，由 HTTPS 反向代理提供浏览器入口。部署、账号维护、Client 配对和远端管理见运维文档。

## 开发验证

```sh
python3 scripts/check-workflow-supply-chain.py
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
cargo +1.98.0 test --locked --target x86_64-unknown-linux-gnu
(cd web && npm ci && npm run build)
```

## 文档

- [文档总览](docs/README.md)
- [初学者指南](docs/beginner-guide/README.md)
- [实例管理](docs/instance-management.md)
- [Client 配对](docs/simple-pairing.md)
- [远端管理](docs/remote-management.md)
- [部署与运维](docs/operations.md)

代码采用 [Apache License 2.0](LICENSE-APACHE)。
