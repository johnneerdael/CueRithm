import {useEffect,useRef,useState,type ReactNode} from "react";
import {LoaderCircle,X} from "lucide-react";
import type {MaintenanceTask} from "../../use-maintenance-task";

export const pageSize=200;
export function bytes(value:number){
  if(value<1024)return value+" B";
  const units=["KB","MB","GB","TB"];let size=value/1024,index=0;
  while(size>=1024&&index<units.length-1){size/=1024;index++;}
  return size.toFixed(size>=100?0:1)+" "+units[index];
}
export function when(ms:number){return new Date(ms).toLocaleString();}
export function fileName(path:string){return path.split(/[\\/]/).filter(Boolean).pop()??path;}

/** Long result lists render a page at a time so very large libraries stay responsive. */
export function usePaged<T>(items:T[],size=pageSize){
  const [shown,setShown]=useState(size);
  useEffect(()=>setShown(size),[items,size]);
  return {visible:items.slice(0,shown),more:items.length>shown?()=>setShown(count=>count+size):null,remaining:Math.max(0,items.length-shown)};
}
export function ShowMore({more,remaining}:{more:(()=>void)|null;remaining:number}){
  return more?<button className="secondary maint-more" onClick={more}>Show {Math.min(remaining,pageSize)} more ({remaining.toLocaleString()} remaining)</button>:null;
}

export function TaskStatus({task}:{task:MaintenanceTask}){
  const {busy,progress,error}=task;
  return <>
    {error?<div role="alert" className="error-banner">{error}<button onClick={task.dismiss}>Dismiss</button></div>:null}
    {busy?<div role="status" className="maint-status"><LoaderCircle className="spin" size={15}/><span>{busy}{progress?` ${progress.completed.toLocaleString()}${progress.total!==null?" of "+progress.total.toLocaleString():""}`:""}</span>
      {progress?.path?<small title={progress.path}>{fileName(progress.path)}</small>:null}<button className="secondary" onClick={task.cancel}>Cancel</button></div>:null}
  </>;
}

export function StaleNotice({stale,reason,what}:{stale:boolean;reason:string|null;what:string}){
  return stale?<div role="alert" className="maint-stale"><strong>This {what} is out of date.</strong> {reason??"The library or files changed since it was made."} Scan again before relying on it.</div>:null;
}

/**
 * Modal confirmation for removals and other irreversible actions. Focus starts on Cancel so Enter cannot
 * confirm by accident; Escape cancels.
 */
export function ConfirmDialog({title,children,confirmLabel,danger,acknowledge,busy,onConfirm,onCancel}:{title:string;children:ReactNode;confirmLabel:string;danger?:boolean;acknowledge?:string;busy?:boolean;onConfirm:()=>void;onCancel:()=>void}){
  const [acknowledged,setAcknowledged]=useState(!acknowledge),cancel=useRef<HTMLButtonElement>(null);
  useEffect(()=>{cancel.current?.focus();},[]);
  useEffect(()=>{const key=(event:KeyboardEvent)=>{if(event.key==="Escape"&&!busy)onCancel();};window.addEventListener("keydown",key);return()=>window.removeEventListener("keydown",key);},[busy,onCancel]);
  return <div className="maint-scrim"><div className="maint-dialog" role="alertdialog" aria-modal="true" aria-labelledby="maint-dialog-title">
    <div className="maint-dialog-head"><h2 id="maint-dialog-title">{title}</h2><button className="icon-button" aria-label="Close" disabled={busy} onClick={onCancel}><X size={16}/></button></div>
    <div className="maint-dialog-body">{children}</div>
    {acknowledge?<label className="maint-ack"><input type="checkbox" checked={acknowledged} onChange={e=>setAcknowledged(e.target.checked)}/>{acknowledge}</label>:null}
    <div className="maint-dialog-actions"><button ref={cancel} className="secondary" disabled={busy} onClick={onCancel}>Cancel</button>
      <button className={danger?"primary maint-danger":"primary"} disabled={busy||!acknowledged} onClick={onConfirm}>{confirmLabel}</button></div>
  </div></div>;
}

const operationStates:Record<string,string>={prepared:"Interrupted — recovery pending",conflict:"Conflict — needs attention",committed:"Completed",undone:"Restored",rolled_back:"Rolled back"};
export function stateLabel(state:string){return operationStates[state]??state;}
/** Prepared or conflicted journals and unfinished cleanup need the user's attention and a retry. */
export function needsRecovery(state:string,cleanupPending=false){return state==="prepared"||state==="conflict"||cleanupPending;}
export function RecoveryNote({state,error,cleanupPending}:{state:string;error:string|null;cleanupPending:boolean}){
  return <>{error?<p role="alert" className="maint-error-text">{error}</p>:null}{cleanupPending?<p className="maint-warn-text">Cleanup is unfinished. Retry to complete it; reported sizes are not proof that storage was released.</p>:null}</>;
}
