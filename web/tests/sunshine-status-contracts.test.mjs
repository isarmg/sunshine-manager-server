import assert from 'node:assert/strict';
import {test} from 'node:test';
import {originalModule} from './fixtures/original-module.mjs';
import {HookHost,walk,textContent} from './fixtures/hook-host.mjs';
const {SunshineControls}=await originalModule(new URL('../src/SunshineControls.tsx',import.meta.url),{
 '@xcss/web/admin-ui':'export const Button="button",FormField="field",TextField="input";',
 '@xcss/web/admin-ui/i18n':'export function t(zh,en){return en}',
 './ConfigSelect':'export const ConfigSelect="select"',
 './api':'export function supportsPendingPairingListing(){return true}'
});
function fixture(section){
 const result=section==='service'?{kind:'service_status_read',state:'running'}:{kind:'diagnostics_read',snapshot:{api_reachable:true,authentication_accepted:true,sunshine_version:'2026.914.233613',platform:'linux',service_state:'running'}};
 const action=section==='service'?'sunshine.service.read':'sunshine.diagnostics.read';
 const old={operation_id:'read-old',action,state:'succeeded',created_at_micros:1,updated_at_micros:5,result,reconciliation:null};
 const failed={operation_id:'read-failed',action,state:'failed',created_at_micros:20,updated_at_micros:25,result:{kind:'rejected',reason:'sunshine_unavailable'},reconciliation:null};
 const host=new HookHost(SunshineControls,{device:{id:'fixture',capabilities:{protocol:'xscs-management/1',service_control:true,diagnostics:true}},operations:[old,failed],busy:false,blocked:false,section,submit:async()=>true,confirm(){}});
 const tree=()=>host.render();const alerts=()=>walk(tree(),node=>node.props.role==='alert');
 return {host,old,failed,result,tree,alerts};
}
for(const section of ['service','diagnostics'])test(`${section} keeps old evidence but exposes a failed fresh read`,()=>{
 const f=fixture(section);assert.equal(f.alerts().length,1);
 const text=textContent(f.tree());
 assert.match(text,/running/);
 assert.match(text,section==='service'?/Last reported state:/ : /Could not refresh diagnostics/);
});
for(const section of ['service','diagnostics'])test(`${section} clears failure only in favor of a newer attempt and distinguishes pending from confirmed state`,()=>{
 const f=fixture(section);assert.equal(f.alerts().length,1);
 const pending={...f.failed,operation_id:'newer-read',state:'pending',created_at_micros:30,updated_at_micros:30,result:null};
 // Older entries appear later and have a later metadata timestamp. Request order wins.
 f.host.props.operations=[pending,f.old,{...f.failed,updated_at_micros:100}];
 assert.equal(f.alerts().length,0);assert.match(textContent(f.tree()),/Waiting for/);assert.match(textContent(f.tree()),/running/);
 const result=section==='service'?{kind:'service_status_read',state:'stopped'}:{...f.result,snapshot:{...f.result.snapshot,api_reachable:false,service_state:'stopped'}};
 f.host.props.operations=[{...f.failed,updated_at_micros:100},{...pending,state:'succeeded',updated_at_micros:40,result},f.old];
 assert.equal(f.alerts().length,0);assert.doesNotMatch(textContent(f.tree()),/Waiting for/);assert.match(textContent(f.tree()),/stopped/);
});
test('unrelated failed reads do not obscure confirmed service status and a newer service control clears the older warning',()=>{
 const f=fixture('service');
 const control={...f.old,operation_id:'control',action:'sunshine.service.control',created_at_micros:30,updated_at_micros:35,result:{kind:'service_controlled',action:'stop',state:'stopped'}};
 f.host.props.operations=[{...f.failed,action:'sunshine.diagnostics.read',created_at_micros:50},f.failed,control,f.old];
 assert.equal(f.alerts().length,0);assert.match(textContent(f.tree()),/Current state:  stopped/);
});
