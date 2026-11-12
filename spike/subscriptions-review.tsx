// Development-only IPC fixtures for the production subscription page.
import { mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { CssBaseline, ThemeProvider } from "@mui/material";
import { HashRouter } from "react-router-dom";
import type { Model, Subscription, SubscriptionPreview } from "../src/bindings";
import { theme } from "../src/theme";
import { Layout } from "../src/components/Layout";
import { ApplyResultProvider } from "../src/components/ApplyResultSnackbar";
import { Subscriptions } from "../src/pages/Subscriptions";

const now=Math.floor(Date.now()/1000);
const node=(name:string)=>({name,proxy_type:"ss",server:"example.test",raw:{name,type:"ss",server:"example.test",port:443,cipher:"aes-128-gcm",password:"demo"}});
const subscription=(id:string,name:string,count:number):Subscription=>({id,name,url:`https://${id}.example/subscribe?token=demo-only`,nodes:Array.from({length:count},(_,i)=>node(`日本 ${i+1}`)),last_updated:now-1800,last_result:{Ok:{node_count:count}},update_interval_mins:60,fallback_exit:null,fallback_listener:null,info:{usage:{upload:0,download:126.4*1024**3,total:300*1024**3,expire:now+18*86400},last_success:now-1800,history:[]}});
let model:Model={subscriptions:[subscription("north","北辰网络",54),subscription("mountain","远山备用",16),{...subscription("private","自建中转",8),info:null,update_interval_mins:null}],manual_nodes:[],listeners:[],exits:[],node_groups:[],process_routing:{enabled:false,rules:[],exit_prompt:true},settings:{autostart:false,core_path:null,controller_port:19097,delay_test_interval_mins:10,delay_test_url:"https://example.test/",node_monitor:{enabled:false,members:[],interval_secs:300,fail_threshold:2,latency_limit_ms:1000,notify_recovery:true}}};
model.subscriptions[1].last_result={Failed:{reason:"下载超时"}};
let deleted:Model|null=null;
const outcome={Reloaded:{cut:0,uncut:0,failed_listeners:[]}};
mockIPC(async(command,payload)=>{
  const args=payload as Record<string,any>;
  if(command==="get_model")return structuredClone(model);
  if(command==="get_status")return {core:"Stopped",last_apply:null};
  if(command==="get_update_status")return {state:{kind:"Unsupported"},current_version:"0.8.4",last_checked_unix:null};
  if(command==="preview_subscription"){
    const s=args.s as Subscription;
    await new Promise(r=>setTimeout(r,450));
    if(s.id==="mountain"&&!s.fallback_listener)throw "下载超时";
    const updated={...s,nodes:s.nodes.length?s.nodes:Array.from({length:24},(_,i)=>node(`新加坡 ${i+1}`))};
    return {subscription:updated,base:"fixture",added:s.nodes.length?[]:updated.nodes.map(n=>n.name),removed:[],affected:[],skipped_tailscale:0,via_proxy:!!s.fallback_listener} satisfies SubscriptionPreview;
  }
  if(command==="commit_subscription_preview"){
    const p=args.preview as SubscriptionPreview,s=p.subscription;
    const record={at:now,result:{Ok:{node_count:s.nodes.length}},added:p.added,removed:p.removed,via_proxy:p.via_proxy,apply:outcome};
    const next={...s,last_updated:now,last_result:record.result,info:{usage:s.info?.usage??null,last_success:now,history:[record,...s.info?.history??[]]}};
    model.subscriptions=model.subscriptions.filter(x=>x.id!==s.id).concat(next);return outcome;
  }
  if(command==="save_subscription"){model.subscriptions=model.subscriptions.map(s=>s.id===args.s.id?args.s:s);return outcome;}
  if(command==="delete_subscription_replacing"){deleted=structuredClone(model);model.subscriptions=model.subscriptions.filter(s=>s.id!==args.id);return {token:"demo-undo",outcome};}
  if(command==="restore_subscription"){if(deleted)model=deleted;return outcome;}
  if(command.startsWith("plugin:"))return null;
  throw `验收环境未提供：${command}`;
},{shouldMockEvents:true});
createRoot(document.getElementById("root")!).render(<ThemeProvider theme={theme} defaultMode="light"><CssBaseline/><QueryClientProvider client={new QueryClient()}><ApplyResultProvider><HashRouter><Layout><Subscriptions/></Layout></HashRouter></ApplyResultProvider></QueryClientProvider></ThemeProvider>);
