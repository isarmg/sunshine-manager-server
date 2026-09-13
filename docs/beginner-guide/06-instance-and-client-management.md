# 06. 实例与 Client 管理

实例是一个受管 Sunshine Client，不是 Sunshine API endpoint。列表显示名称、注册/撤销状态、Client 在线、
Sunshine 可达性、配置状态、能力和最后心跳。

未注册实例可取消配对；取消后再次删除会永久移除残留记录。更换授权码可重新开放配对，并强制旧 Client
失效。已注册实例可改名或永久撤销 credential；撤销后重新接入应创建新实例。

Client 首次连接报告操作系统、Client/Sunshine 版本、是否允许重启和完整 managed fields。服务器不查询或
保存 Moonlight 客户端列表、Sunshine apps、Sunshine 密码或本机证书。
