import { t } from "@sarmg/admin-ui/i18n";
import { InstanceNameField } from "@sarmg/admin-shell";
import {useState,type FormEvent} from "react";
import {Button,Dialog,ErrorState,FormField,TextField} from "@sarmg/admin-ui";
import {useAdminApplication,errorRequestId} from "@sarmg/admin-shell";
import {CURRENT_API_PREFIX,isTicket,type Ticket} from "./api";
export function TicketPanel({ticket}:{ticket:Ticket}){
 return <section className="sarmg-content-panel"><h2>{t("实例授权码", "Instance authorization code")}</h2><p>{t("这是该实例的长期授权码，服务端会加密保存并允许之后查看或更换。更换后客户端必须重新配对。请勿放入命令行、日志或聊天。", "This is the instance's long-lived authorization code. The server stores it encrypted and lets you view or change it later. Changing it requires the client to pair again. Never place it in command lines, logs, or chats.")}</p>
 <dl><dt>{t("管理端标识", "Manager ID")}</dt><dd>{ticket.manager_id}</dd><dt>{t("设备标识", "Device ID")}</dt><dd>{ticket.device.id}</dd><dt>{t("授权码", "Authorization code")}</dt><dd><code className="sunshine-token">{ticket.token}</code></dd></dl>
 <p>{t("客户端只需填写服务器地址、此配对码，以及本机 Sunshine HTTPS 回环地址和账号密码；设备标识自动获取。管理服务器仍要求系统信任且名称匹配的 HTTPS。本机 Sunshine 连接不校验证书身份，但严格限制为回环 IP、不使用代理且不跟随重定向。服务器不接收 Sunshine 密码。", "Enter the server address, this pairing code, and the local Sunshine HTTPS loopback address and credentials. Device IDs are resolved automatically. The manager server still requires system-trusted HTTPS with name verification. The local Sunshine connection does not verify certificate identity, but is restricted to loopback IPs, bypasses proxies, and does not follow redirects. The server never receives the Sunshine password.")}</p></section>;
}
export function DeviceRegistrationDialog({close,created}:{close():void;created(ticket:Ticket):void}){
 const{client}=useAdminApplication();const[pending,setPending]=useState(false);const[failure,setFailure]=useState<{requestId?:string}|null>(null);
 async function submit(event:FormEvent<HTMLFormElement>){event.preventDefault();if(pending)return;const name=String(new FormData(event.currentTarget).get("name")??"").trim();setPending(true);setFailure(null);
 try{created(await client.request(`${CURRENT_API_PREFIX}/sunshine/devices`,isTicket,{method:"POST",body:JSON.stringify({name})}));}catch(error){setFailure({requestId:errorRequestId(error)})}finally{setPending(false)}}
 return <Dialog title={t("新建 Sunshine 实例", "Create Sunshine instance")} description={t("注册安装在 Sunshine 主机上的独立 客户端；不会安装 Sunshine 或改变串流链路。", "Register an independent client on the Sunshine host. This does not install Sunshine or change the streaming connection.")} onClose={()=>{if(!pending)close()}}>
 <form onSubmit={event=>void submit(event)} aria-busy={pending}>{failure&&<ErrorState requestId={failure.requestId}>{t("创建未能确认，请先刷新实例列表核对。", "Creation could not be confirmed. Refresh the instance list and check first.")}</ErrorState>}
 <FormField label={t("实例名称", "Instance name")}><InstanceNameField name="name" required title={t("最多 32 个字符", "Up to 32 characters")} readOnly={pending} data-sarmg-initial-focus/></FormField><p>{t("最多 32 个字符。", "Up to 32 characters.")}</p>
 <div className="sarmg-actions"><Button disabled={pending} onClick={close}>{t("取消", "Cancel")}</Button><Button type="submit" disabled={pending}>{t("创建实例", "Create instance")}</Button></div></form></Dialog>;
}
