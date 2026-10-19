import {useCallback,useEffect,useRef,useState} from "react";
import type {MaintenanceBridge,} from "./maintenance-api";
import type {MaintenanceProgress} from "./maintenance-types";
import {errorMessage} from "./api";

export interface TaskProgress {completed:number;total:number|null;path:string|null}
type Kind=MaintenanceProgress["kind"];

// Native errors carry a code; a cancelled operation is the user's own choice, not a failure to report.
function cancelled(error:unknown){return typeof error==="object"&&error!==null&&"code" in error&&(error as {code:unknown}).code==="cancelled";}

/**
 * Runs one maintenance operation at a time with a fresh operation ID. A cancelled operation keeps the slot
 * until its original promise settles, so `busy` only clears after the native task has released it.
 */
export function useMaintenanceTask(bridge:Pick<MaintenanceBridge,"onProgress"|"cancel">){
  const [busy,setBusy]=useState<string|null>(null),[progress,setProgress]=useState<TaskProgress|null>(null),[error,setError]=useState<string|null>(null);
  const current=useRef<{id:string;kind:Kind|null}|null>(null),alive=useRef(true);
  useEffect(()=>{
    alive.current=true;let off:(()=>void)|undefined,gone=false;
    void bridge.onProgress(event=>{
      const active=current.current;
      if(!alive.current||!active||event.operation_id!==active.id||(active.kind&&event.kind!==active.kind))return;
      const value=event.progress as {completed:number;total:number|null;current_path?:string|null};
      setProgress({completed:value.completed,total:value.total,path:value.current_path??null});
    }).then(unlisten=>{if(gone)unlisten();else off=unlisten;}).catch(()=>{});
    return()=>{alive.current=false;gone=true;off?.();const active=current.current;if(active)void bridge.cancel(active.id).catch(()=>{});};
  },[bridge]);
  const run=useCallback(async<T,>(label:string,kind:Kind|null,job:(operationId:string)=>Promise<T>):Promise<T|undefined>=>{
    if(current.current)return undefined;
    const id=crypto.randomUUID();current.current={id,kind};setBusy(label);setError(null);setProgress(null);
    try{return await job(id);}
    catch(e){if(alive.current&&!cancelled(e))setError(errorMessage(e));return undefined;}
    finally{current.current=null;if(alive.current){setBusy(null);setProgress(null);}}
  },[]);
  const cancel=useCallback(()=>{const active=current.current;if(active)void bridge.cancel(active.id).catch(()=>{});},[bridge]);
  const dismiss=useCallback(()=>setError(null),[]);
  return {busy,progress,error,run,cancel,dismiss,setError};
}
export type MaintenanceTask=ReturnType<typeof useMaintenanceTask>;
