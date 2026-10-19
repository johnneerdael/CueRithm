import {useCallback,useEffect,useState} from "react";
import {RefreshCw,Undo2,EyeOff} from "lucide-react";
import {errorMessage} from "../../api";
import type {MaintenanceBridge} from "../../maintenance-api";
import type {DuplicateIgnores,DuplicateMergeHistory,DuplicateRecovery} from "../../maintenance-types";
import type {MaintenanceTask} from "../../use-maintenance-task";
import {needsRecovery,stateLabel,when} from "./shared";

/** Merge history with Undo, interrupted-operation recovery, and ignored tracks. */
export default function DuplicateExtras({bridge,task,changeTick,onLibraryChanged,onNotice}:{bridge:MaintenanceBridge;task:MaintenanceTask;changeTick:number;onLibraryChanged:()=>void;onNotice:(text:string)=>void}){
  const [history,setHistory]=useState<DuplicateMergeHistory[]>([]),[recovery,setRecovery]=useState<DuplicateRecovery[]>([]),[ignores,setIgnores]=useState<DuplicateIgnores|null>(null);
  const load=useCallback(async()=>{
    try{const [h,r,i]=await Promise.all([bridge.mergeHistory(),bridge.recovery(),bridge.ignores()]);setHistory(h);setRecovery(r);setIgnores(i);}
    catch(e){task.setError(errorMessage(e));}
  },[bridge,task.setError]);
  useEffect(()=>{void load();},[load,changeTick]);
  const undo=(entry:DuplicateMergeHistory)=>void task.run("Undoing merge…",null,async id=>{
    const result=await bridge.undoMerge(id,entry.result.id);
    onNotice(`Merge undone. ${result.restored_ids.length.toLocaleString()} tracks restored and original file bytes returned.`);onLibraryChanged();await load();
  });
  const retry=(entry:DuplicateRecovery)=>void task.run("Retrying recovery…",null,async id=>{setRecovery(await bridge.retryRecovery(id,entry.id));onLibraryChanged();});
  const restore=(trackId:string)=>ignores&&void task.run("Restoring track to duplicate scans…",null,async()=>{
    try{setIgnores(await bridge.setIgnores([trackId],false,ignores.stamp));}
    catch(e){await load();throw e;}
  });
  const pending=recovery.filter(r=>needsRecovery(r.state));
  const merges=history.filter(h=>!h.result.undo_of);
  return <>
    {pending.length?<section className="maint-history" aria-label="Interrupted merges"><h2>Needs recovery</h2>
      <p className="cleanup-note">An interrupted merge or undo was found. Recovery never overwrites a file you changed afterwards; fix the conflict, then retry.</p>
      {pending.map(r=><article key={r.id} className="maint-history-row"><div><strong>{r.kind.replace(/_/g," ")}</strong><small>{stateLabel(r.state)} · {when(r.created)}</small>{r.error?<p role="alert" className="maint-error-text">{r.error}</p>:null}</div>
        <div className="maint-row-actions"><button className="secondary" disabled={!!task.busy} onClick={()=>retry(r)}><RefreshCw size={13}/>Retry recovery</button></div></article>)}</section>:null}
    <section className="maint-history" aria-label="Merge history"><h2>Merge history</h2>
      {!merges.length?<p className="cleanup-note">Applied merges appear here. No music files are deleted by merging.</p>:merges.map(h=><article key={h.result.id} className="maint-history-row">
        <div><strong>{h.result.archived_ids.length.toLocaleString()} archived · {h.result.updated_ids.length.toLocaleString()} updated</strong><small>{when(h.created)}{h.undone_by?" · Undone":""}</small></div>
        <div className="maint-row-actions"><button className="secondary" disabled={!!task.busy||!!h.undone_by} onClick={()=>undo(h)}><Undo2 size={13}/>Undo</button></div></article>)}
    </section>
    <section className="maint-history" aria-label="Ignored tracks"><h2>Ignored tracks</h2>
      {!ignores?.tracks.length?<p className="cleanup-note">Tracks you ignore in a review are left out of duplicate groups. They appear here so you can bring them back.</p>:ignores.tracks.map(t=><article key={t.id} className="maint-history-row">
        <div><strong>{t.artist} – {t.title}</strong><small title={t.display_path}>{t.display_path}</small></div>
        <div className="maint-row-actions"><button className="secondary" disabled={!!task.busy} onClick={()=>restore(t.id)}><EyeOff size={13}/>Stop ignoring</button></div></article>)}
    </section>
  </>;
}
