import { createAdministratorApiClient } from "@sarmg/admin-web";
import { isErrorEnvelope, type ErrorEnvelope } from "@sarmg/contracts";
import { isApiClientError } from "@sarmg/http-client";

export const CURRENT_API_PREFIX = "/api/v2";
export const adminApi = createAdministratorApiClient();

export type Snapshot = {revision:string;sunshine_version:string;fields:Record<string,string>;effectiveness:"pending_verification"|"awaiting_restart"};
export type Capabilities={protocol:string;client_version:string;os:string;sunshine_version:string;restart_allowed:boolean;managed_fields:string[];application_management:boolean;application_host_commands_allowed:boolean;moonlight_pairing_management:boolean;diagnostics:boolean;maintenance:boolean;service_control:boolean};
export type DeviceInfo = {id:string;name:string;registered:boolean;pairing_pending:boolean;revoked:boolean;client_online:boolean;sunshine_reachable:boolean|null;configuration_state:string;snapshot:Snapshot|null;capabilities:Capabilities|null;last_seen_at_micros:number|null};
export type Ticket={device:DeviceInfo;manager_id:string;token:string};
export type ClientAuthorization={manager_id:string;device_id:string;authorization_code:string};
export type ConfigFieldDefinition={key:string;kind:"text"|"integer"|"boolean"|"select";minimum?:number;maximum?:number;maximum_length?:number;options?:string[];requires_restart:boolean;supported_sunshine_versions:string[];operating_systems:string[];prerequisite?:string};
export type ApplicationRef={fingerprint:string};
export type PreparationCommand={do:string;undo:string;elevated:boolean};
export type ApplicationSpec={name:string;output:string;cmd:string;"working-dir":string;"exclude-global-prep-cmd":boolean;elevated:boolean;"auto-detach":boolean;"wait-all":boolean;"exit-timeout":number;"prep-cmd":PreparationCommand[];detached:string[];"image-path":string};
export type ApplicationView={reference:ApplicationRef;specification:ApplicationSpec};
export type ApplicationsSnapshot={revision:string;applications:ApplicationView[]};
export type PairedClient={uuid:string;name:string;enabled:boolean};
export type PairedClientsSnapshot={revision:string;clients:PairedClient[]};
export type LogCursor={revision:string;before_offset:number};
export type LogPage={revision:string;text:string;start_offset:number;end_offset:number;total_bytes:number;previous:LogCursor|null;redacted:boolean};
export type DiagnosticSnapshot={sunshine_version:string|null;platform:string|null;api_reachable:boolean;authentication_accepted:boolean;service_state:string;configuration_revision:string|null};
export type DriverStatus={installed:boolean;version:string|null;required_version:string|null;error:string|null};
export type VirtualInputStatus={virtualhid:DriverStatus;vigembus:DriverStatus};
export type Command=
 |{kind:"read_config"}
 |{kind:"patch_config";expected_revision:string;set:Record<string,string|number|boolean>;remove:string[];restart_policy:"manual"}
 |{kind:"restart";expected_revision:string;administrator_confirmed:true}
 |{kind:"list_applications"}
 |{kind:"save_application";expected_revision:string;target:ApplicationRef|null;application:ApplicationSpec;administrator_confirmed_host_commands:boolean}
 |{kind:"delete_application";expected_revision:string;target:ApplicationRef;administrator_confirmed:true}
 |{kind:"close_application";administrator_confirmed:true}
 |{kind:"upload_cover";key:string;png_base64:string;administrator_confirmed:true}
 |{kind:"submit_pairing_pin";pairing_id:string;pin:string;name:string}
 |{kind:"list_paired_clients"}
 |{kind:"set_paired_client_enabled";uuid:string;enabled:boolean;administrator_confirmed:true}
 |{kind:"unpair_client";uuid:string;administrator_confirmed:true}
 |{kind:"unpair_all_clients";administrator_confirmed:true}
 |{kind:"read_logs";cursor:LogCursor|null;limit_bytes:number}
 |{kind:"read_diagnostics"}
 |{kind:"read_virtual_input_status"}
 |{kind:"run_maintenance";action:"reset_display_persistence"|"reset_portal_token";administrator_confirmed:true}
 |{kind:"read_service_status"}
 |{kind:"control_service";action:"start"|"stop"|"restart";administrator_confirmed:true};
export type Report={kind:string;snapshot?:Snapshot|ApplicationsSnapshot|PairedClientsSnapshot|DiagnosticSnapshot;page?:LogPage;status?:VirtualInputStatus;state?:string;path?:string;action?:string;reason?:string;actual_revision?:string};
export type Operation={operation_id:string;device_id:string;action:string;state:"pending"|"running"|"succeeded"|"failed"|"unknown"|"dead_letter"|"resolved";attempt:number;created_at_micros:number;updated_at_micros:number;result:Report|null;reconciliation:Report|null;resolution:string|null;status_reason?:string|null};

export function record(value:unknown):value is Record<string,unknown>{return typeof value==="object"&&value!==null&&!Array.isArray(value)}
export function isSnapshot(value:unknown):value is Snapshot{return record(value)&&typeof value.revision==="string"&&/^[a-f0-9]{64}$/.test(value.revision)&&typeof value.sunshine_version==="string"&&record(value.fields)&&Object.values(value.fields).every(field=>typeof field==="string")&&(value.effectiveness==="pending_verification"||value.effectiveness==="awaiting_restart")}
export function isDevice(value:unknown):value is DeviceInfo{
 if(!record(value)||typeof value.id!=="string"||typeof value.name!=="string"||typeof value.registered!=="boolean"||typeof value.pairing_pending!=="boolean"||typeof value.revoked!=="boolean"||typeof value.client_online!=="boolean"||(value.sunshine_reachable!==null&&typeof value.sunshine_reachable!=="boolean")||typeof value.configuration_state!=="string"||(value.snapshot!==null&&!isSnapshot(value.snapshot))||(value.last_seen_at_micros!==null&&!Number.isSafeInteger(value.last_seen_at_micros)))return false;
 if(value.capabilities===null)return true;
 const capabilities=value.capabilities;
 return record(capabilities)&&typeof capabilities.protocol==="string"&&typeof capabilities.client_version==="string"&&typeof capabilities.os==="string"&&typeof capabilities.sunshine_version==="string"&&typeof capabilities.restart_allowed==="boolean"&&Array.isArray(capabilities.managed_fields)&&capabilities.managed_fields.every(field=>typeof field==="string")&&["application_management","application_host_commands_allowed","moonlight_pairing_management","diagnostics","maintenance","service_control"].every(key=>typeof capabilities[key]==="boolean");
}
export function isDevices(value:unknown):value is DeviceInfo[]{return Array.isArray(value)&&value.every(isDevice)}
export function isTicket(value:unknown):value is Ticket{return record(value)&&isDevice(value.device)&&typeof value.manager_id==="string"&&typeof value.token==="string"&&/^[a-z0-9]{36}$/.test(value.token)}
export function isClientAuthorization(value:unknown):value is ClientAuthorization{return record(value)&&Object.keys(value).length===3&&typeof value.manager_id==="string"&&/^[0-9a-f-]{36}$/.test(value.manager_id)&&typeof value.device_id==="string"&&/^[0-9a-f-]{36}$/.test(value.device_id)&&typeof value.authorization_code==="string"&&/^[a-z0-9]{36}$/.test(value.authorization_code)}
export function isConfigFieldDefinition(value:unknown):value is ConfigFieldDefinition{return record(value)&&typeof value.key==="string"&&["text","integer","boolean","select"].includes(String(value.kind))&&(!("minimum" in value)||Number.isSafeInteger(value.minimum))&&(!("maximum" in value)||Number.isSafeInteger(value.maximum))&&(!("maximum_length" in value)||Number.isSafeInteger(value.maximum_length))&&(!("options" in value)||(Array.isArray(value.options)&&value.options.every(option=>typeof option==="string")))&&typeof value.requires_restart==="boolean"&&Array.isArray(value.supported_sunshine_versions)&&value.supported_sunshine_versions.every(version=>typeof version==="string")&&Array.isArray(value.operating_systems)&&value.operating_systems.every(os=>typeof os==="string")&&(!("prerequisite" in value)||typeof value.prerequisite==="string")}
export function isConfigFieldDefinitions(value:unknown):value is ConfigFieldDefinition[]{return Array.isArray(value)&&value.every(isConfigFieldDefinition)&&new Set(value.map(field=>field.key)).size===value.length}
function isRevision(value:unknown):value is string{return typeof value==="string"&&/^[a-f0-9]{64}$/.test(value)}
function isApplicationSpec(value:unknown):value is ApplicationSpec{return record(value)&&typeof value.name==="string"&&typeof value.output==="string"&&typeof value.cmd==="string"&&typeof value["working-dir"]==="string"&&typeof value["exclude-global-prep-cmd"]==="boolean"&&typeof value.elevated==="boolean"&&typeof value["auto-detach"]==="boolean"&&typeof value["wait-all"]==="boolean"&&Number.isSafeInteger(value["exit-timeout"])&&Array.isArray(value["prep-cmd"])&&value["prep-cmd"].every(item=>record(item)&&typeof item.do==="string"&&typeof item.undo==="string"&&typeof item.elevated==="boolean")&&Array.isArray(value.detached)&&value.detached.every(item=>typeof item==="string")&&typeof value["image-path"]==="string"}
function isApplicationsSnapshot(value:unknown):value is ApplicationsSnapshot{return record(value)&&isRevision(value.revision)&&Array.isArray(value.applications)&&value.applications.every(app=>record(app)&&record(app.reference)&&isRevision(app.reference.fingerprint)&&isApplicationSpec(app.specification))}
function isPairedClientsSnapshot(value:unknown):value is PairedClientsSnapshot{return record(value)&&isRevision(value.revision)&&Array.isArray(value.clients)&&value.clients.every(client=>record(client)&&typeof client.uuid==="string"&&typeof client.name==="string"&&typeof client.enabled==="boolean")}
function isLogPage(value:unknown):value is LogPage{return record(value)&&isRevision(value.revision)&&typeof value.text==="string"&&Number.isSafeInteger(value.start_offset)&&Number.isSafeInteger(value.end_offset)&&Number.isSafeInteger(value.total_bytes)&&(value.previous===null||(record(value.previous)&&isRevision(value.previous.revision)&&Number.isSafeInteger(value.previous.before_offset)))&&typeof value.redacted==="boolean"}
function isDiagnosticSnapshot(value:unknown):value is DiagnosticSnapshot{return record(value)&&(value.sunshine_version===null||typeof value.sunshine_version==="string")&&(value.platform===null||typeof value.platform==="string")&&typeof value.api_reachable==="boolean"&&typeof value.authentication_accepted==="boolean"&&typeof value.service_state==="string"&&(value.configuration_revision===null||isRevision(value.configuration_revision))}
function isDriver(value:unknown):value is DriverStatus{return record(value)&&typeof value.installed==="boolean"&&(value.version===null||typeof value.version==="string")&&(value.required_version===null||typeof value.required_version==="string")&&(value.error===null||typeof value.error==="string")}
function isVirtualInput(value:unknown):value is VirtualInputStatus{return record(value)&&isDriver(value.virtualhid)&&isDriver(value.vigembus)}
function isReport(value:unknown):value is Report{
 if(!record(value)||typeof value.kind!=="string")return false;
 switch(value.kind){
  case "config_read":case "config_saved":case "restart_acknowledged":return isSnapshot(value.snapshot);
  case "applications_read":case "application_saved":case "application_deleted":return isApplicationsSnapshot(value.snapshot);
  case "paired_clients_read":case "paired_client_updated":return isPairedClientsSnapshot(value.snapshot);
  case "logs_read":return isLogPage(value.page);
  case "diagnostics_read":return isDiagnosticSnapshot(value.snapshot);
  case "virtual_input_status_read":return isVirtualInput(value.status);
  case "service_status_read":return typeof value.state==="string";
  case "service_controlled":return typeof value.action==="string"&&typeof value.state==="string";
  case "cover_uploaded":return typeof value.path==="string";
  case "maintenance_completed":return typeof value.action==="string";
  case "conflict":return isRevision(value.actual_revision);
  case "rejected":case "unknown":return typeof value.reason==="string";
  case "application_closed":case "pairing_pin_submitted":return true;
  default:return false;
 }
}
export function isOperation(value:unknown):value is Operation{return record(value)&&typeof value.operation_id==="string"&&typeof value.device_id==="string"&&typeof value.action==="string"&&typeof value.state==="string"&&["pending","running","succeeded","failed","unknown","dead_letter","resolved"].includes(value.state)&&Number.isSafeInteger(value.attempt)&&Number.isSafeInteger(value.created_at_micros)&&Number.isSafeInteger(value.updated_at_micros)&&(value.result===null||isReport(value.result))&&(value.reconciliation===null||isReport(value.reconciliation))&&(value.resolution===null||typeof value.resolution==="string")&&(value.status_reason===undefined||value.status_reason===null||typeof value.status_reason==="string")}
export function isOperations(value:unknown):value is Operation[]{return Array.isArray(value)&&value.every(isOperation)}
export function currentErrorEnvelope(error:unknown):ErrorEnvelope|undefined{if(!isApiClientError(error))return undefined;return isErrorEnvelope(error.envelope)?error.envelope:undefined}
