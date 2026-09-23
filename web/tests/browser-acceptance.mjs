import { checkWebLanguage } from "./language.mjs";
import { checkHeaderActions, checkHeaderLogout } from "./header-actions.mjs";
import assert from "node:assert/strict";
import {chromium,firefox,expect} from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import {preview} from "vite";
import {randomUUID} from "node:crypto";
const session={authenticated:true,user_id:"A".repeat(43),username:"admin",role:"admin",csrf_token:"A".repeat(43)};
const configFields=[];
async function assertColumnContentAlignment(table){
 const offsets=await table.evaluate(element=>{const textStart=cell=>{const walker=document.createTreeWalker(cell,NodeFilter.SHOW_TEXT);let text;while((text=walker.nextNode())&&!text.textContent.trim()){}if(!text)throw new Error("table cell has no visible text");const range=document.createRange();range.selectNodeContents(text);return range.getBoundingClientRect().left};const contentStart=cell=>textStart(cell);const headings=[...element.querySelectorAll("thead th")],values=[...element.querySelector("tbody tr").children];if(headings.length!==values.length)throw new Error("table column count mismatch");return headings.map((heading,index)=>Math.abs(textStart(heading)-contentStart(values[index])))});
 assert.ok(offsets.every(offset=>offset<0.5),`column content offsets: ${JSON.stringify(offsets)}`);
}
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
      assert.deepEqual(req.postDataJSON(),{name:"新实例"});
      if(posts===1)return route.fulfill({status:503,json:{code:"service_unavailable",retryable:true,message:"SECRET details",request_id:"create-123"}});
      const device={id:randomUUID(),name:"新实例",registered:false,pairing_pending:true,revoked:false,client_online:false,sunshine_reachable:null,configuration_state:"unknown",snapshot:null,capabilities:null,last_seen_at_micros:null};
      devices.push(device);return route.fulfill({status:201,json:{device,manager_id:randomUUID(),token:"b".repeat(36)}});
     }return route.fulfill({json:devices});
    }
    if(path.match(/\/sunshine\/devices\/[^/]+$/)&&req.method()==="PATCH"){
     devices[0].name=req.postDataJSON().name;
     return route.fulfill({json:devices[0]});
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
     return route.fulfill({json:{manager_id:randomUUID(),device_id:devices[0]?.id??randomUUID(),authorization_code:"b".repeat(36)}});
    }
    if(path.endsWith("/tasks/calendar"))return route.fulfill({json:{today:"2030-01-02"}});
    if(path.endsWith("/tasks/summary"))return route.fulfill({json:{blocking_count:0}});
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
   await expect(page.getByRole("alert")).toContainText("create-123");await expect(page.locator("body")).not.toContainText("SECRET details");
   await page.getByRole("button",{name:"新建实例",exact:true}).click();
   await expect(page.getByRole("button",{name:"关闭通知",exact:true})).toBeVisible();
   await expect(page).toHaveURL(/#instances$/);
   await expect(page.getByRole("button",{name:"实例列表",exact:true})).toHaveAttribute("aria-pressed","true");
   await expect(page.getByRole("link",{name:"选择实例 新实例"})).toHaveText("新实例");
   await expect(page.getByRole("button",{name:"关闭通知",exact:true})).toHaveCount(0,{timeout:7000});
   await expect(page.getByRole("complementary")).toHaveCount(0);
   await page.getByRole("button",{name:"实例列表",exact:true}).click();
   await expect(page.getByRole("link",{name:"选择实例 新实例"})).toHaveText("新实例");
   const table=page.getByRole("table",{name:"Sunshine 实例列表"});
   const statistics=page.getByRole("table",{name:"实例统计"});
   await expect(statistics.getByRole("columnheader")).toHaveText(["统计项","总数 / 在线"]);
   await expect(statistics.getByRole("rowheader")).toHaveText(["总数","Windows","Linux","macOS"]);
   await expect(statistics.locator("tbody td")).toHaveText(["1 / 0","0 / 0","0 / 0","0 / 0"]);
   await expect(table.locator("tbody tr")).toHaveCount(1);
   await expect(table.getByRole("columnheader")).toHaveText(["实例名称","注册状态","客户端 状态","Sunshine 接口","配置状态","操作系统/架构","最近连接","删除"]);
   await expect(table.locator("tbody td")).toHaveText(["等待配对","离线","未知","尚未核对","尚未上报","尚未连接","删除"]);
   await expect(table).not.toContainText(devices[0].id);
   await expect(table).not.toContainText("b".repeat(36));
   const instanceNameStyle=await table.getByRole("link",{name:"选择实例 新实例"}).evaluate(element=>({color:getComputedStyle(element).color,parentColor:getComputedStyle(element.parentElement).color,decoration:getComputedStyle(element).textDecorationLine}));
   assert.equal(instanceNameStyle.color,instanceNameStyle.parentColor);
   assert.equal(instanceNameStyle.decoration,"none");
   assert.ok((await table.locator("th, td").evaluateAll(elements=>elements.map(element=>getComputedStyle(element).textAlign))).every(value=>value==="left"));
   await assertColumnContentAlignment(table);
   assert.ok((await table.locator(".sarmg-actions").evaluateAll(elements=>elements.map(element=>getComputedStyle(element).justifyContent))).every(value=>value==="flex-start"));
   assert.equal(await table.locator("tbody tr").evaluate(row=>getComputedStyle(row).display),"table-row");
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
   assert.deepEqual((await new AxeBuilder({page}).withTags(["wcag2a","wcag2aa","wcag21aa"]).analyze()).violations,[]);
   failNextAuthorization=true;
   await page.getByRole("link",{name:"选择实例 新实例"}).click();
   await expect(page.getByRole("button",{name:"详细信息",exact:true})).toHaveAttribute("aria-pressed","true");
   const pairingDetails=page.getByRole("region",{name:"配对账户信息"});
   await expect(pairingDetails.getByRole("heading",{name:"新实例",exact:true})).toBeVisible();
   await expect(pairingDetails).toContainText(devices[0].id);
   await expect(page.getByText("等待配对",{exact:true})).toBeVisible();
   await expect(page.getByLabel("Sunshine 密码",{exact:true})).toHaveCount(0);
   await expect(page.getByText("账户名",{exact:true})).toHaveCount(1);
   await expect(page.getByText("账户",{exact:true})).toHaveCount(1);
   await expect(page.getByText("密码",{exact:true})).toHaveCount(1);
   await expect(page.getByRole("region",{name:"实例设置"})).toBeVisible();
   await expect(page.getByRole("region",{name:"Sunshine 状态"})).toBeVisible();
   await expect(page.getByRole("alert").filter({hasText:"无法读取实例授权码"})).toContainText("authorization-failure-123");
   await page.getByRole("group",{name:"全局操作"}).getByRole("button",{name:"刷新",exact:true}).click();
   await expect.poll(()=>authorizationAttempts).toBeGreaterThan(1);
   await expect(page.locator(".sunshine-workspace .sunshine-token").first()).toHaveText("b".repeat(36));
   await expect(page.getByRole("alert")).toHaveCount(0);
   const nameInput=page.getByRole("textbox",{name:"实例名称",exact:true});
   await nameInput.fill("正在编辑的名称");
   devices[0].name="另一浏览器保存的名称";
   await page.getByRole("group",{name:"全局操作"}).getByRole("button",{name:"刷新",exact:true}).click();
   await expect(pairingDetails.getByRole("heading",{name:devices[0].name,exact:true})).toBeVisible();
   await expect(nameInput).toHaveValue("正在编辑的名称");
   await nameInput.fill("新实例");
   await page.getByRole("button",{name:"保存名称",exact:true}).click();
   await expect.poll(()=>devices[0].name).toBe("新实例");
   await expect(page.getByRole("button",{name:"保存名称",exact:true})).toBeDisabled();
   failNextTasks=true;
   await page.getByRole("group",{name:"全局操作"}).getByRole("button",{name:"刷新",exact:true}).click();
   await expect(page.getByRole("alert")).toContainText("tasks-failure-123");
   await expect(page.locator("body")).not.toContainText("SECRET tasks");
   await expect(page.getByRole("alert")).toHaveCount(0,{timeout:5000});
   await nameInput.fill("\uFEFF"+"a".repeat(32));
   await expect(page.getByRole("alert")).toContainText("实例名称须为 1–32 个字符");
   await expect(page.getByRole("button",{name:"保存名称",exact:true})).toBeDisabled();
   assert.equal(devices[0].name,"新实例");
   await nameInput.fill("\uFEFF新实例");
   await expect(page.getByRole("alert")).toHaveCount(0);
   await expect(page.getByRole("button",{name:"保存名称",exact:true})).toBeEnabled();
   await page.getByRole("button",{name:"保存名称",exact:true}).click();
   await expect.poll(()=>devices[0].name).toBe("\uFEFF新实例");
   await nameInput.fill("新实例");
   await page.getByRole("button",{name:"保存名称",exact:true}).click();
   await expect.poll(()=>devices[0].name).toBe("新实例");
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
   await page.getByRole("button",{name:"取消配对",exact:true}).click();
   await page.getByRole("dialog").getByRole("button",{name:"取消",exact:true}).click();
   assert.equal(devices[0].pairing_pending,true);
   await expect(page.getByText("等待配对",{exact:true})).toBeVisible();
   await page.getByRole("button",{name:"取消配对",exact:true}).click();await page.getByRole("button",{name:"确认",exact:true}).click();
   await expect(page.getByText("配对已取消",{exact:true})).toBeVisible();
   await expect(page.locator(".sunshine-workspace .sunshine-token").first()).toHaveText("b".repeat(36));
   await checkWebLanguage(page, {"routes":[["instances","Instance list"],["details","Details"],["logs","Logs"]],"names":["新实例"]});
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
