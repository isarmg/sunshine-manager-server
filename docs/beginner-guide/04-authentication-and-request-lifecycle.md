# 04. 认证与请求生命周期

浏览器管理员认证由 Foundation 提供：username/password 登录、Secure HttpOnly SameSite Cookie、Session
恢复/退出、CSRF 和严格 Origin/Host 校验。实例授权码、Client credential 与管理员密码不是同一种凭据。

管理员创建实例时得到长期授权码；Server 保存可解密密文和查找摘要。Client 用它完成配对并得到随机
credential，后续通过 Bearer credential 查询 identity 和升级 WSS。更换授权码立即撤销旧 credential，
要求 Client 重新配对。

Client 通道只允许 loopback ingress，要求 `X-Forwarded-Proto: https` 并拒绝 Origin/Cookie。WSS 还要求
精确 subprotocol。任何 binding、版本、字段集或在线 session 不匹配都失败关闭。
