import {useEffect,useRef,useState} from "react";
import {LoaderCircle,ShieldCheck} from "lucide-react";
import {nativeMaintenance} from "../maintenance-api";
import type {MaintenanceBridge} from "../maintenance-api";
import {nativeMissingRepair,noMissingRepair} from "../missing-repair-api";
import type {MissingRepairBridge} from "../missing-repair-api";
import type {MaintenanceProgress} from "../maintenance-types";
import type {PlaylistWorkspace} from "../playlist-types";
import {errorMessage} from "../api";
import {MaintenanceContext} from "./MaintenanceShared";
import type {TaskKind} from "./MaintenanceShared";
import MaintenanceDuplicates from "./MaintenanceDuplicates";
import MaintenanceUnused from "./MaintenanceUnused";
import MaintenanceBroken from "./MaintenanceBroken";
import MaintenanceHistory from "./MaintenanceHistory";
export default function Maintenance({api=nativeMaintenance,repair,chooseFolder,workspace,onChanged,revision}:{api?:MaintenanceBridge;repair?:MissingRepairBridge;chooseFolder:()=>Promise<string|null>;workspace:PlaylistWorkspace|null;onChanged:()=>void;revision?:number}){
 const missingRepair=repair??(api===nativeMaintenance?nativeMissingRepair:noMissingRepair);
 const [tab,setTab]=useState("duplicates"),[status,setStatus]=useState<string|null>(null),[error,setError]=useState<string|null>(null),[message,setMessage]=useState<string|null>(null),[progress,setProgress]=useState<MaintenanceProgress|null>(null),[cancelling,setCancelling]=useState(false);
 const [attention,setAttention]=useState(0);
 const [historyVersion,setHistoryVersion]=useState(0),[externalChange,setExternalChange]=useState(0);
 const alive=useRef(true),active=useRef<{id:string;kind:TaskKind;cancelled:boolean}|null>(null),changed=useRef(onChanged);changed.current=onChanged;
 useEffect(()=>{alive.current=true;const events=api.onChange(()=>{if(!alive.current)return;setHistoryVersion(v=>v+1);if(!active.current)setExternalChange(v=>v+1);changed.current();});void events.catch(e=>{if(alive.current)setError(errorMessage(e));});return()=>{alive.current=false;const task=active.current;if(task){task.cancelled=true;void api.cancel(task.id).catch(()=>{});}void events.then(stop=>stop()).catch(()=>{});};},[api]);
 const previousRevision=useRef(revision);
 useEffect(()=>{if(previousRevision.current!==revision){previousRevision.current=revision;if(!active.current)setExternalChange(v=>v+1);}},[revision]);
 const run=async<T,>(kind:TaskKind,label:string,work:(id:string)=>Promise<T>):Promise<T|undefined>=>{
  if(active.current||!alive.current)return;const task={id:crypto.randomUUID(),kind,cancelled:false};active.current=task;setStatus(label);setError(null);setMessage(null);setProgress(null);setCancelling(false);let stop:(()=>void)|undefined;
  try{stop=await api.onProgress(event=>{if(alive.current&&active.current===task&&event.operation_id===task.id&&event.kind===kind)setProgress(event);});if(!alive.current||task.cancelled)return;const result=await work(task.id);if(alive.current&&!task.cancelled)return result;}
  catch(e){if(alive.current)setError(errorMessage(e));}
  finally{stop?.();if(active.current===task)active.current=null;if(alive.current){setStatus(null);setProgress(null);setCancelling(false);setHistoryVersion(v=>v+1);if(task.cancelled)setMessage("Operation ended after cancellation. Check history for any committed changes.");}}
 };
 const cancel=async()=>{const task=active.current;if(!task||task.cancelled)return;task.cancelled=true;setCancelling(true);try{await api.cancel(task.id);}catch(e){if(alive.current)setError(errorMessage(e));}};
 return <section className="maintenance-view"><header className="view-title"><div><div className="eyebrow">LIBRARY TOOLS</div><h1>Maintenance</h1><p>Find duplicates, check files and review unused music.</p></div><ShieldCheck size={26} className="maintenance-heading-icon"/></header>
  <div role="tablist" aria-label="Maintenance tools" className="cleanup-tabs">{[["duplicates","Duplicates"],["broken","Missing & broken"],["unused","Unused files"],["history","History & recovery"]].map(([id,label])=><button key={id} role="tab" aria-selected={tab===id} disabled={!!status} onClick={()=>setTab(id)}>{label}</button>)}</div>
  <MaintenanceContext.Provider value={{api,repair:missingRepair,busy:!!status,externalChange,historyVersion,run,changed:()=>changed.current(),message:setMessage,openTab:setTab}}><div className="maintenance-body">
   {attention&&tab!=="history"?<button className="maintenance-warning" disabled={!!status} onClick={()=>setTab("history")}>{attention} recovery {attention===1?"item needs":"items need"} attention</button>:null}
   {error?<div role="alert" className="error-banner">{error}</div>:null}{message?<p role="status" className="maintenance-result">{message}</p>:null}
   {status?<div role="status" className="maintenance-progress"><LoaderCircle size={15} className="spin"/><strong>{cancelling?"Cancelling; waiting for the operation to finish…":status}</strong>{progress?<><span>{progress.progress.completed} / {progress.progress.total??"…"}</span>{progress.progress.total?<progress max={progress.progress.total} value={progress.progress.completed}/>:null}</>:null}<button className="secondary" disabled={cancelling} onClick={()=>void cancel()}>Cancel operation</button></div>:null}
   {tab==="duplicates"?<MaintenanceDuplicates workspace={workspace}/>:tab==="unused"?<MaintenanceUnused chooseFolder={chooseFolder}/>:tab==="broken"?<MaintenanceBroken chooseFolder={chooseFolder}/>:null}
   <div hidden={tab!=="history"}><MaintenanceHistory onAttention={setAttention}/></div>
  </div></MaintenanceContext.Provider></section>;
}
