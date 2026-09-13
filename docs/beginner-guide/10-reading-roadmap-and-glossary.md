# 10. 阅读路线与术语

推荐顺序：`protocol/` → `src/http.rs` → `src/db.rs` → `src/operations.rs` → `web/src/` → release scripts。

| 术语 | 当前含义 |
|---|---|
| 实例 | Server 中对应一个独立 Sunshine Client 的记录 |
| 授权码 | 每实例长期秘密；可查看/更换，更换后必须重新配对 |
| Binding | manager/device/installation 三元身份 |
| credential | Client 配对后取得的随机 Bearer 秘密，Server 只存摘要 |
| snapshot | Client 回报的白名单配置和 revision |
| operation | 持久化的 read/patch/restart 管理意图 |
| unknown | 副作用是否发生无法证明，需 inspect-only 与人工核对 |
| inspect-only | 只查看 Client 已有执行证据，绝不能再次执行 |

读完后应能解释：为什么 Server 不需要 Sunshine 密码，为什么语言切换与任务执行无关，为什么授权码轮换
需要重新配对，以及为什么 unknown 不能自动重试。
