import { t } from "../shell/i18n.js";
import {createSarmgAdminApplication,errorRequestId,useAdminApplication,InstancePageNavigation,type InstancePage} from "../shell/index.js";
import {EmptyState,ErrorState,LoadingState} from "@sarmg/admin-ui";
import {useEffect,useState} from "react";
import {CURRENT_API_PREFIX,adminApi,isDevices,type DeviceInfo,type Ticket} from "./api";
import {DeviceRegistrationDialog} from "./DeviceRegistrationDialog";
import {ClientWorkspace} from "./ClientWorkspace";
import {DeviceInstances} from "./DeviceInstances";
import { InstanceHeaderActions } from "../shell/index.js";
function currentPage():InstancePage{return ["details","logs"].includes(window.location.hash.slice(1))?window.location.hash.slice(1) as InstancePage:"instances"}
function DevicesPage(){
 const{client}=useAdminApplication();const[devices,setDevices]=useState<DeviceInfo[]|null>(null);const[failure,setFailure]=useState<{requestId?:string}|null>(null);
 const[generation,setGeneration]=useState(0);const[creating,setCreating]=useState(false);const[selected,setSelected]=useState<string|null>(null);const[ticket,setTicket]=useState<Ticket|null>(null);
 const[page,setPage]=useState(currentPage);
 useEffect(()=>{const changed=()=>setPage(currentPage());window.addEventListener("hashchange",changed);return()=>window.removeEventListener("hashchange",changed)},[]);
 useEffect(()=>{const controller=new AbortController();let active=true;async function load(){try{const values=await client.request(CURRENT_API_PREFIX+"/sunshine/devices",isDevices,{signal:controller.signal});if(active){setDevices(values);setFailure(null)}}catch(error){if(active)setFailure({requestId:errorRequestId(error)})}}void load();const timer=setInterval(()=>void load(),5000);return()=>{active=false;controller.abort();clearInterval(timer)}},[client,generation]);
 const refresh=()=>setGeneration(value=>value+1);const device=devices?.find(value=>value.id===selected)??devices?.[0];
 return <section className="sarmg-content-stack"><InstanceHeaderActions create={()=>setCreating(true)} refresh={refresh}/><InstancePageNavigation page={page} detailsDisabled={!device} navigate={value=>{window.location.hash=value}}/><h1 className="sarmg-visually-hidden">{t("Sunshine 设备管理", "Sunshine device management")}</h1>
 {failure&&<ErrorState requestId={failure.requestId} onRetry={refresh}>{t("无法加载设备列表", "Unable to load devices")}</ErrorState>}
 {page==="instances"?<section className="sarmg-content-stack" aria-label={t("Sunshine 实例", "Sunshine instances")}><h2>{t("实例总览", "Instance overview")}</h2>{devices===null?<LoadingState>{t("正在加载设备…", "Loading devices…")}</LoadingState>:<DeviceInstances devices={devices} select={id=>{setSelected(id);window.location.hash="details"}}/>}</section>
 :device?<><h2>{device.name}</h2><ClientWorkspace key={device.id} device={device} page={page} changed={refresh} ticket={ticket?.device.id===device.id?ticket:null}/></>:devices===null?<LoadingState>{t("正在加载设备…", "Loading devices…")}</LoadingState>:<EmptyState>{t("暂无实例，请新建并注册 客户端。", "No instances yet. Create an instance and register an client.")}</EmptyState>}
 {creating&&<DeviceRegistrationDialog close={()=>setCreating(false)} created={value=>{setTicket(value);setSelected(value.device.id);setCreating(false);window.location.hash="details";refresh()}}/>}</section>
}
export default createSarmgAdminApplication({product:{name:"Sunshine Manager"},client:adminApi,navigation:[],routes:<DevicesPage/>});
