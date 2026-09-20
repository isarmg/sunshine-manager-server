import { t } from "@sarmg/admin-ui/i18n";
import {createSarmgAdminApplication,errorRequestId,useAdminApplication,InstancePageNavigation,type InstancePage} from "@sarmg/admin-shell";
import {EmptyState,ErrorState,LoadingState,Table} from "@sarmg/admin-ui";
import {useEffect,useState} from "react";
import {CURRENT_API_PREFIX,adminApi,isConfigFieldDefinitions,isDevices,type ConfigFieldDefinition,type DeviceInfo,type Ticket} from "./api";
import {DeviceRegistrationDialog} from "./DeviceRegistrationDialog";
import {ClientWorkspace} from "./ClientWorkspace";
import {DeviceInstances} from "./DeviceInstances";
import { InstanceHeaderActions } from "@sarmg/admin-shell";
function currentRoute():{page:InstancePage;deviceId:string|null}{const[page,id]=window.location.hash.slice(1).split("/");return{page:["details","logs"].includes(page)?page as InstancePage:"instances",deviceId:id&&/^[0-9a-f-]{36}$/.test(id)?id:null}}
function DevicesPage(){
 const{client}=useAdminApplication();const[devices,setDevices]=useState<DeviceInfo[]|null>(null);const[deviceFailure,setDeviceFailure]=useState<{requestId?:string}|null>(null);const[fieldFailure,setFieldFailure]=useState<{requestId?:string}|null>(null);
 const[fieldDefinitions,setFieldDefinitions]=useState<ConfigFieldDefinition[]|null>(null);
 const[generation,setGeneration]=useState(0);const[creating,setCreating]=useState(false);const[selected,setSelected]=useState<string|null>(()=>currentRoute().deviceId);const[ticket,setTicket]=useState<Ticket|null>(null);
 const[page,setPage]=useState(()=>currentRoute().page);
 useEffect(()=>{const changed=()=>{const route=currentRoute();setPage(route.page);if(route.deviceId)setSelected(route.deviceId)};window.addEventListener("hashchange",changed);return()=>window.removeEventListener("hashchange",changed)},[]);
 useEffect(()=>{const controller=new AbortController();let active=true;let timer:number|undefined;async function load(){try{const values=await client.request(CURRENT_API_PREFIX+"/sunshine/devices",isDevices,{signal:controller.signal});if(active){setDevices(values);setDeviceFailure(null)}}catch(error){if(active)setDeviceFailure({requestId:errorRequestId(error)})}finally{if(active)timer=window.setTimeout(load,5000)}}void load();return()=>{active=false;controller.abort();if(timer!==undefined)clearTimeout(timer)}},[client,generation]);
 useEffect(()=>{const controller=new AbortController();let active=true;setFieldFailure(null);void client.request(CURRENT_API_PREFIX+"/sunshine/config-fields",isConfigFieldDefinitions,{signal:controller.signal}).then(value=>{if(active){setFieldDefinitions(value);setFieldFailure(null)}}).catch(error=>{if(active)setFieldFailure({requestId:errorRequestId(error)})});return()=>{active=false;controller.abort()}},[client,generation]);
 const refresh=()=>setGeneration(value=>value+1);const device=selected===null?devices?.[0]:devices?.find(value=>value.id===selected);
 const statistics=[
  [t("总数","Total"),devices??[]],
  ["Windows",(devices??[]).filter(value=>value.capabilities?.os.toLowerCase().includes("windows"))],
  ["Linux",(devices??[]).filter(value=>value.capabilities?.os.toLowerCase().includes("linux"))],
  ["macOS",(devices??[]).filter(value=>{const os=value.capabilities?.os.toLowerCase()??"";return os.includes("macos")||os.includes("mac os")||os.includes("darwin")})],
 ] as const;
 return <section className="sarmg-content-stack"><InstanceHeaderActions create={()=>setCreating(true)} refresh={refresh}/><InstancePageNavigation page={page} detailsDisabled={!device} navigate={value=>{window.location.hash=device&&value!=="instances"?`${value}/${device.id}`:value}}/><h1 className="sarmg-visually-hidden">{t("Sunshine 设备管理", "Sunshine device management")}</h1>
 {deviceFailure&&<ErrorState requestId={deviceFailure.requestId} onRetry={refresh}>{t("无法加载设备列表", "Unable to load devices")}</ErrorState>}
 {fieldFailure&&<ErrorState requestId={fieldFailure.requestId} onRetry={refresh}>{t("无法加载配置字段定义", "Unable to load configuration field definitions")}</ErrorState>}
 {page==="instances"?<div className="sarmg-content-stack" aria-label={t("Sunshine 实例", "Sunshine instances")}>
  <section className="sarmg-content-stack"><h2>{t("统计", "Statistics")}</h2>{devices===null?<LoadingState>{t("正在加载设备…", "Loading devices…")}</LoadingState>:<Table aria-label={t("实例统计", "Instance statistics")}><thead><tr><th>{t("统计项", "Metric")}</th><th>{t("总数 / 在线", "Total / online")}</th></tr></thead><tbody>{statistics.map(([label,values])=><tr key={label}><th scope="row">{label}</th><td>{values.length} / {values.filter(value=>value.client_online).length}</td></tr>)}</tbody></Table>}</section>
  <section className="sarmg-content-stack"><h2>{t("实例列表", "Instance list")}</h2>{devices===null?<LoadingState>{t("正在加载设备…", "Loading devices…")}</LoadingState>:<DeviceInstances devices={devices} select={id=>{setSelected(id);window.location.hash=`details/${id}`}} removed={id=>{if(selected===id)setSelected(null);if(ticket?.device.id===id)setTicket(null);refresh()}}/>}</section>
 </div>
 :device?<><h2>{device.name}</h2><ClientWorkspace key={device.id} device={device} fieldDefinitions={fieldDefinitions} refreshSignal={generation} page={page} changed={refresh} removed={()=>{setSelected(null);setTicket(null);window.location.hash="instances";refresh()}} ticket={ticket?.device.id===device.id?ticket:null}/></>:devices===null?<LoadingState>{t("正在加载设备…", "Loading devices…")}</LoadingState>:<EmptyState>{t("暂无实例，请新建并注册 客户端。", "No instances yet. Create an instance and register an client.")}</EmptyState>}
 {creating&&<DeviceRegistrationDialog close={()=>setCreating(false)} created={value=>{setTicket(value);setSelected(value.device.id);setCreating(false);window.location.hash=`details/${value.device.id}`;refresh()}}/>}</section>
}
export default createSarmgAdminApplication({product:{name:"Sunshine Manager"},client:adminApi,navigation:[],routes:<DevicesPage/>});
