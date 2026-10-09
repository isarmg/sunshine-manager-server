# 本地启动 xscs

本地源码服务（不是 systemd 生产安装）只监听 `127.0.0.1:18104`。它不安装或启动 Sunshine，也不自动注册 Client 实例。

```sh
npm --prefix web ci
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=2 web/node_modules/.bin/xcss-build-server --config foundation-web-build.json --mode development --no-install
node scripts/local-service.mjs init
node scripts/local-service.mjs start
node scripts/local-service.mjs status
node scripts/local-service.mjs stop
```

浏览器访问 `http://127.0.0.1:18104`。管理员为 `admin`；显式 `init` 随机生成密码，保存于权限为 0600 的 `.runtime/local-service/login.txt`，不写日志或 Git。独立数据库及密钥保留在忽略目录 `.runtime/local-service/`，重启使用同一份状态。不要删除或替换现有密钥文件。

修改前端时，保持后端运行，在另一个终端执行 `npm run dev --prefix web`，访问 `http://127.0.0.1:5173`。开发服务支持热更新，将 `/api` 请求代理到 18104，并保留浏览器 Host 以通过登录和 CSRF 同源检查。使用同一份管理员账号密码；按 Ctrl+C 停止前端开发服务。

只在回环监听下使用产品已有的开发 HTTP Session 模式。停止命令验证记录的进程启动时间和二进制路径，只向匹配的本地服务发送 SIGTERM；不强制杀死其他占用端口的进程。

当前管理 Web 的默认字体来自 正式发布并锁定的 Server Foundation 字体包：英文为 Maple Mono Normal NL 正体（非斜体、非手写、无连字），中日文为 Maple Mono NL CN 正体。全部字体分片在界面显示前由本地服务加载完成，不依赖访问者安装字体或外部 CDN。

Web 与 Server 使用 Foundation 同一构建入口。默认二进制自带管理页面；本地启动默认使用内嵌资源；执行 `node scripts/local-service.mjs start --directory-web` 显式选择开发目录 `web/dist`，修改后重新构建 Web 即生效，无需重编译 Rust。也可运行 Vite 开发服务器获得源码热更新。正式 source-bound 二进制拒绝目录资源模式。

构建产物默认位于 `target/x86_64-unknown-linux-gnu/debug/`。若设置自定义 `CARGO_TARGET_DIR`，启动/状态/停止时使用 `XCSS_LOCAL_SERVER_BINARY` 指定同一绝对二进制路径。

本地 `init` 创建 `.runtime/local-service/server.json`（0600）与 `data/`（0700），已有配置拒绝覆盖。`start` 只读取当前配置及已初始化状态，缺失时失败；`status` 不创建目录。运行日志位于 `data/logs/`，默认有界轮转，配置中的秘密不输出到日志。
