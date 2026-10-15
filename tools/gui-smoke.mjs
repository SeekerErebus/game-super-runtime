// Optional browser acceptance check. Requires Node >=22 and Chromium, no npm deps.
import {spawn} from 'node:child_process';
import {mkdtemp, readFile, mkdir, writeFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve, join} from 'node:path';
import assert from 'node:assert/strict';

const root=await mkdtemp(join(tmpdir(),'gsr-gui-'));
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const children=[];
let ws;
async function until(fn,label){const end=Date.now()+12000;while(Date.now()<end){const result=await fn();if(result)return result;await wait(50);}throw Error('Timed out: '+label);}
try {
  const gsr=spawn(resolve(process.argv[2]||'target/debug/game-super-runtime'),['demo','--edit-gui','--no-open'],{env:{...process.env,GSR_CONFIG_DIR:join(root,'config'),XDG_STATE_HOME:join(root,'state')}});children.push(gsr);
  let output='';gsr.stdout.on('data',b=>output+=b);gsr.stderr.on('data',b=>process.stderr.write(b));
  const url=await until(()=>output.match(/http:\/\/127\.0\.0\.1:\d+\/[a-f0-9]+\//)?.[0],'editor URL');
  const chrome=spawn(process.env.CHROMIUM||'chromium',['--headless','--no-sandbox','--disable-gpu','--disable-dev-shm-usage','--disable-background-networking','--no-first-run','--no-default-browser-check','--remote-debugging-port=0','--user-data-dir='+join(root,'browser'),'about:blank'],{stdio:['ignore','ignore','pipe']});children.push(chrome);
  let chromeErrors='';chrome.stderr.on('data',b=>chromeErrors+=b);
  const port=await until(async()=>{try{return (await readFile(join(root,'browser','DevToolsActivePort'),'utf8')).split('\n')[0];}catch{if(chrome.exitCode!==null)throw Error(chromeErrors);return null;}},'Chromium');
  const pages=await (await fetch('http://127.0.0.1:'+port+'/json/list')).json();
  ws=new WebSocket(pages.find(p=>p.type==='page'&&p.url==='about:blank').webSocketDebuggerUrl);await new Promise((res,rej)=>{ws.onopen=res;ws.onerror=rej;});
  let serial=0;const pending=new Map(),errors=[];
  ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);if(!p)return;pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}if(m.method==='Runtime.exceptionThrown')errors.push(m.params.exceptionDetails);};
  const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++serial;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
  const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
  await send('Page.enable');await send('Runtime.enable');await send('Emulation.setDeviceMetricsOverride',{width:1280,height:900,deviceScaleFactor:1,mobile:false});await send('Page.navigate',{url});
  await until(()=>evaluate('document.getElementById("status")?.textContent.startsWith("Loaded.")'),'config loaded').catch(async e=>{console.error('Browser diagnostics:',errors,await evaluate('JSON.stringify({href:location.href,html:document.documentElement.outerHTML})'));throw e;});
  await evaluate('document.getElementById("name").value="Browser tested";document.getElementById("name").dispatchEvent(new Event("input",{bubbles:true}));document.querySelector("[data-tab=performance]").click();document.getElementById("mangohud").click();document.getElementById("save").click();');
  await until(()=>evaluate('document.getElementById("status").textContent.startsWith("Saved.")'),'save form');
  const yaml=await readFile(join(root,'config/demo.yaml'),'utf8');assert.match(yaml,/name: Browser tested/);assert.match(yaml,/mangohud: true/);
  await evaluate('document.querySelector("[data-tab=yaml]").click()');await wait(100);
  await evaluate('document.getElementById("yaml").value="unknown_field: true";document.getElementById("yaml").dispatchEvent(new Event("input",{bubbles:true}));document.getElementById("save").click();');
  await until(()=>evaluate('document.getElementById("status").classList.contains("error")'),'invalid YAML rejection');
  assert.equal(await readFile(join(root,'config/demo.yaml'),'utf8'),yaml);
  await evaluate('dirty=false;document.getElementById("reload").click()');await until(()=>evaluate('document.getElementById("status").textContent.startsWith("Loaded.")'),'reload');
  await evaluate('document.querySelector("[data-tab=launch]").click()');await wait(100);
  const shot=await send('Page.captureScreenshot',{format:'png'});await mkdir('target/qa',{recursive:true});await writeFile('target/qa/editor.png',Buffer.from(shot.data,'base64'));
  await send('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});await wait(100);
  assert.equal(await evaluate('document.documentElement.scrollWidth<=window.innerWidth'),true,'mobile layout must fit');
  assert.deepEqual(errors,[],'no browser JavaScript errors');
  await evaluate('document.getElementById("close").click()');await until(()=>gsr.exitCode===0,'editor shutdown');
  console.log('GUI browser smoke passed: form save, YAML rejection, reload, mobile layout, clean shutdown. Screenshot: target/qa/editor.png');
} finally {
  if(ws)ws.close();
  for(const child of children){if(child.exitCode===null)child.kill('SIGTERM');}
  await wait(300);
  await rm(root,{recursive:true,force:true,maxRetries:3,retryDelay:100});
}
