import assert from "node:assert/strict";
import {chromium,firefox,expect} from "@playwright/test";
import {preview} from "vite";
import {randomUUID} from "node:crypto";
const session={authenticated:true,user_id:"A".repeat(43),username:"admin",role:"admin",csrf_token:"A".repeat(43)};
const configFields=[
 {key:"sunshine_name",kind:"text",maximum_length:32,requires_restart:true,supported_sunshine_versions:["2026.914.233613"],operating_systems:["linux_x86_64"]},
 {key:"qp",kind:"integer",minimum:0,maximum:51,requires_restart:true,supported_sunshine_versions:["2026.914.233613"],operating_systems:["linux_x86_64"]},
];
const server=await preview({preview:{host:"127.0.0.1",port:0,strictPort:true}});
try {for(const engine of [chromium,firefox]){
 const browser=await engine.launch();
 try {
  const page=await browser.newPage({ locale: "zh-CN" });const commands=[];const errors=[];const operations=[];
  page.on("pageerror",e=>errors.push(e.message));
  const app={reference:{fingerprint:"c".repeat(64)},specification:{name:"Steam",output:"",cmd:"","working-dir":"","exclude-global-prep-cmd":false,elevated:false,"auto-detach":false,"wait-all":false,"exit-timeout":5,"prep-cmd":[],detached:[],"image-path":""}};
  const device={id:randomUUID(),name:"游戏主机",registered:true,pairing_pending:false,revoked:false,client_online:true,sunshine_reachable:true,configuration_state:"pending_verification",last_seen_at_micros:Date.now()*1000,capabilities:{protocol:"sunshine-management/2",client_version:"0.2.0",os:"linux_x86_64",sunshine_version:"2026.914.233613",restart_allowed:true,managed_fields:["sunshine_name","qp"],application_management:true,application_host_commands_allowed:true,moonlight_pairing_management:true,diagnostics:true,maintenance:true,service_control:true},snapshot:{revision:"a".repeat(64),sunshine_version:"2026.914.233613",fields:{sunshine_name:"Original",qp:"28"},effectiveness:"pending_verification"}};
  await page.route("**/api/v2/**",async route=>{
   const req=route.request();const path=new URL(req.url()).pathname;
   if(path.endsWith("/sunshine/config-fields"))return route.fulfill({json:configFields});
   if(path.endsWith("/sunshine/devices"))return route.fulfill({json:[device]});
   if(path.endsWith("/authorization"))return route.fulfill({json:{manager_id:randomUUID(),device_id:device.id,authorization_code:"b".repeat(64)}});
   if(path.endsWith("/tasks")){
    if(req.method()==="POST"){
     const command=req.postDataJSON();commands.push(command);
     assert.equal(req.headers()["x-csrf-token"],session.csrf_token);assert.ok(req.headers()["idempotency-key"]);
     const action={read_config:"sunshine.config.read",patch_config:"sunshine.config.patch",restart:"sunshine.restart",list_applications:"sunshine.applications.list",save_application:"sunshine.applications.save",list_paired_clients:"sunshine.pairing.clients.list",read_logs:"sunshine.logs.read",read_diagnostics:"sunshine.diagnostics.read",read_virtual_input_status:"sunshine.virtual_input.read",read_service_status:"sunshine.service.read"}[command.kind]??`sunshine.${command.kind}`;
     const snapshot={...device.snapshot,effectiveness:command.kind==="patch_config"?"awaiting_restart":"pending_verification"};
     const result=command.kind==="restart"?{kind:"restart_acknowledged",snapshot}:command.kind==="read_config"?{kind:"config_read",snapshot}:command.kind==="patch_config"?{kind:"config_saved",snapshot}:command.kind==="list_applications"?{kind:"applications_read",snapshot:{revision:"d".repeat(64),applications:[app]}}:command.kind==="save_application"?{kind:"application_saved",snapshot:{revision:"e".repeat(64),applications:[{reference:{fingerprint:"f".repeat(64)},specification:command.application}]}}:command.kind==="list_paired_clients"?{kind:"paired_clients_read",snapshot:{revision:"1".repeat(64),clients:[{uuid:"123e4567-e89b-12d3-a456-426614174000",name:"Moonlight TV",enabled:true}]}}:command.kind==="read_logs"?{kind:"logs_read",page:{revision:"2".repeat(64),text:"Sunshine ready\n",start_offset:0,end_offset:15,total_bytes:15,previous:null,redacted:false}}:command.kind==="read_diagnostics"?{kind:"diagnostics_read",snapshot:{sunshine_version:"2026.914.233613",platform:"linux",api_reachable:true,authentication_accepted:true,service_state:"running",configuration_revision:"a".repeat(64)}}:command.kind==="read_virtual_input_status"?{kind:"virtual_input_status_read",status:{virtualhid:{installed:false,version:null,required_version:null,error:null},vigembus:{installed:false,version:null,required_version:null,error:null}}}:command.kind==="read_service_status"?{kind:"service_status_read",state:"running"}:{kind:"maintenance_completed",action:command.action};
     const operation={operation_id:"op_"+randomUUID(),device_id:device.id,action,state:"succeeded",attempt:1,created_at_micros:Date.now()*1000,updated_at_micros:Date.now()*1000,result,reconciliation:null,resolution:null};
     operations.unshift(operation);return route.fulfill({json:operation});
    }return route.fulfill({json:operations});
   }return route.fulfill({json:session});
  });
  await page.goto("http://127.0.0.1:"+server.httpServer.address().port);
  await page.getByRole("link",{name:"选择实例 游戏主机",exact:true}).click();
  await expect(page.getByRole("button",{name:"详细信息",exact:true})).toHaveAttribute("aria-pressed","true");
  await page.getByLabel("Sunshine 名称", { exact: true }).fill("New name");
  await page.getByRole("button",{name:"预览变更",exact:true}).click();
  await expect(page.getByRole("region",{name:"变更差异预览"})).toContainText("Original");
  await expect(page.getByRole("region",{name:"变更差异预览"})).toContainText("New name");
  assert.equal(commands.length,0);
  await page.getByRole("button",{name:"确认保存配置",exact:true}).click();
  await expect.poll(()=>commands.length).toBe(1);
  assert.deepEqual(commands[0],{kind:"patch_config",expected_revision:"a".repeat(64),set:{sunshine_name:"New name"},remove:[],restart_policy:"manual"});
  await page.getByRole("button",{name:"重启 Sunshine",exact:true}).click();
  const dialog=page.getByRole("dialog",{name:"确认重启 Sunshine"});
  await expect(dialog).toContainText("中断正在进行的串流");assert.equal(commands.length,1);
  await dialog.getByRole("button",{name:"确认",exact:true}).click();
  await expect.poll(()=>commands.length).toBe(2);
  assert.equal(commands[1].administrator_confirmed,true);
  await page.getByRole("button",{name:"刷新应用",exact:true}).click();
  await expect(page.locator("body")).toContainText("Steam");
  await page.getByRole("button",{name:"编辑",exact:true}).click();
  await page.getByLabel("名称",{exact:true}).fill("Steam Remote");
  await page.getByRole("button",{name:"保存应用",exact:true}).click();
  await expect.poll(()=>commands.some(command=>command.kind==="save_application")).toBe(true);
  const save=commands.find(command=>command.kind==="save_application");
  assert.deepEqual(save.target,{fingerprint:"c".repeat(64)});assert.equal(save.expected_revision,"d".repeat(64));
  await page.getByRole("button",{name:"刷新已配对客户端",exact:true}).click();
  await expect(page.locator("body")).toContainText("Moonlight TV");
  await page.getByRole("button",{name:"读取最新日志",exact:true}).click();
  await expect(page.locator("body")).toContainText("Sunshine ready");
  await page.getByRole("button",{name:"读取诊断",exact:true}).click();
  await expect(page.locator("body")).toContainText("2026.914.233613");
  await page.getByRole("button",{name:"刷新服务状态",exact:true}).click();
  await expect(page.locator("body")).toContainText("running");
  await expect(page.getByRole("button",{name:"重置显示设备",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"日志",exact:true}).click();
  await expect(page.locator("body")).toContainText("已确认重启请求");
  assert.deepEqual(errors,[]);console.log(engine.name()+": protocol v2 configuration, applications, pairing, logs, diagnostics and service controls passed");
 }finally{await browser.close();}
}}finally{await new Promise(done=>server.httpServer.close(done));}
