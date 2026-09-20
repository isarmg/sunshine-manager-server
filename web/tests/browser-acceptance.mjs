import { checkWebLanguage } from "./language.mjs";
import { checkHeaderActions, checkHeaderLogout } from "./header-actions.mjs";
import assert from "node:assert/strict";
import {chromium,firefox,expect} from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import {preview} from "vite";
import {randomUUID} from "node:crypto";
const session={authenticated:true,user_id:"A".repeat(43),username:"admin",role:"admin",csrf_token:"A".repeat(43)};
const configFields=[];
const server=await preview({preview:{host:"127.0.0.1",port:0,strictPort:true}});
try {
 for(const engine of [chromium,firefox]){
  const browser=await engine.launch();
  try {
   const context=await browser.newContext({ locale: "zh-CN", viewport:{width:390,height:844}});const page=await context.newPage();const errors=[];const devices=[];let posts=0;let configAttempts=0;let authorizationAttempts=0;let failNextAuthorization=false;let failNextTasks=false;
   page.on("pageerror",e=>errors.push(e.message));
   await page.route("**/api/v2/**",async route=>{
    const req=route.request();const path=new URL(req.url()).pathname;
    if(path.endsWith("/sunshine/config-fields")){configAttempts++;if(configAttempts===1)return route.fulfill({status:503,json:{code:"service_unavailable",retryable:true,message:"SECRET fields",request_id:"fields-failure-123"}});return route.fulfill({json:configFields});}
    if(path.endsWith("/sunshine/devices")){
     if(req.method()==="POST"){
      posts++;assert.equal(req.headers()["x-csrf-token"],session.csrf_token);
      assert.deepEqual(Object.keys(req.postDataJSON()),["name"]);
      if(posts===1)return route.fulfill({status:503,json:{code:"service_unavailable",retryable:true,message:"SECRET details",request_id:"create-123"}});
      const device={id:randomUUID(),name:req.postDataJSON().name,registered:false,pairing_pending:true,revoked:false,client_online:false,sunshine_reachable:null,configuration_state:"unknown",snapshot:null,capabilities:null,last_seen_at_micros:null};
      devices.push(device);return route.fulfill({status:201,json:{device,manager_id:randomUUID(),token:"b".repeat(64)}});
     }return route.fulfill({json:devices});
    }
    if(path.endsWith("/pairing")&&req.method()==="DELETE"){
     devices[0].pairing_pending=false;
     return route.fulfill({status:204})
    }
    if(path.match(/\/sunshine\/devices\/[^/]+$/)&&req.method()==="DELETE"){
     devices.splice(0,1);return route.fulfill({status:204})
    }
    if(path.endsWith("/authorization")){
     authorizationAttempts++;
     if(failNextAuthorization){
      failNextAuthorization=false;
      return route.fulfill({status:503,json:{code:"service_unavailable",retryable:true,message:"SECRET authorization",request_id:"authorization-failure-123"}});
     }
     return route.fulfill({json:{manager_id:randomUUID(),device_id:devices[0]?.id??randomUUID(),authorization_code:"b".repeat(64)}});
    }
    if(path.endsWith("/tasks")){if(failNextTasks){failNextTasks=false;return route.fulfill({status:503,json:{code:"service_unavailable",retryable:true,message:"SECRET tasks",request_id:"tasks-failure-123"}})}return route.fulfill({json:[]});}
    return route.fulfill({json:session});
   });
   await page.goto("http://127.0.0.1:"+server.httpServer.address().port);
   await expect(page.getByRole("alert")).toContainText("fields-failure-123");
   await expect(page.locator("body")).not.toContainText("SECRET fields");
   await page.getByRole("alert").getByRole("button",{name:"重试",exact:true}).click();
   await expect.poll(()=>configAttempts).toBeGreaterThan(1);
   await expect(page.getByRole("alert")).toHaveCount(0);
   await checkHeaderActions(page, "/sunshine/devices");
   const menuToFirst=await page.evaluate(()=>{
    const header=document.querySelector(".sarmg-page-header");const first=[...document.querySelectorAll("h2")].find(node=>node.textContent==="统计");
    if(!header||!first)throw new Error("Sunshine spacing fixture is incomplete");
    return first.getBoundingClientRect().top-header.getBoundingClientRect().bottom;
   });
   assert.ok(Math.abs(menuToFirst-16)<2,String(menuToFirst));
   await page.getByRole("button",{name:"新建实例",exact:true}).click();
   const dialog=page.getByRole("dialog",{name:"新建 Sunshine 实例"});
   const input=dialog.getByLabel("实例名称",{exact:true});
   for(const character of ["a","中","😀"]){await input.fill(character.repeat(32));assert.equal(await input.evaluate(e=>e.checkValidity()),true);}
   await input.fill("名".repeat(33));assert.equal(await input.evaluate(e=>e.checkValidity()),false);
   await input.fill("测试 Sunshine");
   await dialog.getByRole("button",{name:"创建实例",exact:true}).click();
   await expect(dialog.getByRole("alert")).toBeVisible();await expect(page.locator("body")).not.toContainText("SECRET details");
   await dialog.getByRole("button",{name:"创建实例",exact:true}).click();
   await expect(dialog).toHaveCount(0);
   await expect(page.getByRole("complementary")).toHaveCount(0);
   await page.getByRole("button",{name:"实例列表",exact:true}).click();
   await expect(page.getByRole("button",{name:"选择实例 测试 Sunshine"})).toHaveText("测试 Sunshine");
   const table=page.getByRole("table",{name:"Sunshine 实例列表"});
   await expect(table.locator("tbody tr")).toHaveCount(1);
   await expect(table.getByRole("columnheader")).toHaveText(["实例名称","注册状态","客户端 状态","Sunshine 接口","配置状态","操作系统","最近连接","删除"]);
   await expect(table.locator("tbody td")).toHaveText(["等待配对","离线","未知","尚未核对","尚未上报","尚未连接","删除"]);
   assert.equal(await table.locator("tbody tr").evaluate(row=>getComputedStyle(row).display),"table-row");
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
   assert.deepEqual((await new AxeBuilder({page}).withTags(["wcag2a","wcag2aa","wcag21aa"]).analyze()).violations,[]);
   failNextAuthorization=true;
   await page.getByRole("button",{name:"选择实例 测试 Sunshine"}).click();
   await expect(page.getByRole("button",{name:"详细信息",exact:true})).toHaveAttribute("aria-pressed","true");
   await expect(page.getByText("等待配对",{exact:true})).toBeVisible();
   await expect(page.getByLabel("Sunshine 密码",{exact:true})).toHaveCount(0);
   await expect(page.getByText("实例授权码",{exact:true})).toHaveCount(1);
   await expect(page.getByRole("alert")).toContainText("authorization-failure-123");
   await page.getByRole("group",{name:"全局操作"}).getByRole("button",{name:"刷新",exact:true}).click();
   await expect.poll(()=>authorizationAttempts).toBeGreaterThan(1);
   await expect(page.locator(".sunshine-workspace .sunshine-token").first()).toHaveText("b".repeat(64));
   await expect(page.getByRole("alert")).toHaveCount(0);
   failNextTasks=true;
   await page.getByRole("group",{name:"全局操作"}).getByRole("button",{name:"刷新",exact:true}).click();
   await expect(page.getByRole("alert")).toContainText("tasks-failure-123");
   await expect(page.locator("body")).not.toContainText("SECRET tasks");
   await expect(page.getByRole("alert")).toHaveCount(0,{timeout:5000});
   for(const theme of ["dark","light"]){
    await page.getByRole("button",{name:theme==="dark"?"切换到深色模式":"切换到浅色模式",exact:true}).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme",theme);
    const violations=(await new AxeBuilder({page}).withTags(["wcag2a","wcag2aa","wcag21aa"]).analyze()).violations;
    assert.deepEqual(violations,[]);
   }
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
   await expect(page.locator("body")).toContainText("更换后客户端必须重新配对");
   const detailActions=page.getByRole("button",{name:"删除实例",exact:true}).locator("..");
   await detailActions.evaluate(element=>element.click());
   await expect(page.getByRole("dialog")).toHaveCount(0);
   await page.getByRole("button",{name:"取消配对",exact:true}).click();await page.getByRole("button",{name:"确认",exact:true}).click();
   await expect(page.getByText("配对已取消",{exact:true})).toBeVisible();
   await expect(page.locator(".sunshine-workspace .sunshine-token").first()).toHaveText("b".repeat(64));
   await checkWebLanguage(page, {"routes":[["instances","Instance list"],["details","Details"],["logs","Logs"]],"names":["测试 Sunshine"]});
   await page.getByRole("button",{name:"实例列表",exact:true}).click();
   await page.getByRole("button",{name:"删除",exact:true}).click();
   await page.getByRole("button",{name:"取消",exact:true}).click();
   await expect(page.getByRole("button",{name:"确认删除",exact:true})).toHaveCount(0);
   await page.getByRole("button",{name:"删除",exact:true}).click();
   await page.getByRole("button",{name:"确认删除",exact:true}).click();
   await expect(page.getByText("暂无实例，请新建并注册 客户端。",{exact:true})).toBeVisible();
   await checkHeaderLogout(page, session.csrf_token);
   assert.deepEqual(errors,[]);console.log(engine.name()+": Client registration, name limits, CSRF, default appearance, mobile and WCAG passed");
  }finally{await browser.close();}
 }
}finally{await new Promise(done=>server.httpServer.close(done));}
