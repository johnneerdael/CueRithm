import {useCallback,useEffect,useState} from "react";
import {HardDrive,RefreshCw} from "lucide-react";
import {errorMessage} from "../../api";
import type {Bridge} from "../../types";
import type {MaintenanceBridge} from "../../maintenance-api";
import type {RetainedDisposal,RetainedPreview} from "../../maintenance-types";
import type {MaintenanceTask} from "../../use-maintenance-task";
import {ConfirmDialog,ShowMore,bytes,usePaged,when} from "./shared";

/**
 * Original files kept while tagging and cleanup rewrite your audio. They stay until you dispose of them here;
 * originals that still let you Undo a cleanup are offered only when you choose to give up that Undo.
 */
export default function RetainedOriginals({library,bridge,task,changeTick}:{library:Bridge;bridge:MaintenanceBridge;task:MaintenanceTask;changeTick:number}){
  const [storage,setStorage]=useState<{original_files:number;original_bytes:number;pending_cleanup:number}|null>(null),[history,setHistory]=useState<RetainedDisposal[]>([]);
  const [includeUndoable,setIncludeUndoable]=useState(false),[preview,setPreview]=useState<RetainedPreview|null>(null),[confirming,setConfirming]=useState(false),[notice,setNotice]=useState<string|null>(null);
  const load=useCallback(async()=>{
    try{const [s,h]=await Promise.all([library.getTaggingStorage(),bridge.retainedDisposalHistory()]);setStorage(s);setHistory(h);}
    catch(e){task.setError(errorMessage(e));}
  },[library,bridge,task.setError]);
  useEffect(()=>{setPreview(null);void load();},[load,changeTick]);
  const paged=usePaged(preview?.files??[]);
  const review=()=>void task.run("Measuring retained originals…",null,async id=>{setPreview(await bridge.previewRetainedOriginals(id,includeUndoable));setNotice(null);});
  const dispose=()=>preview&&void task.run("Disposing of originals…",null,async id=>{
    const result=await bridge.disposeRetainedOriginals(id,preview.id);
    setPreview(null);
    setNotice(result.cleanup_pending?"Disposal is recorded but some files are still waiting to be deleted. Retry it from the history below.":`Disposed of ${result.file_count.toLocaleString()} retained originals (${bytes(result.size_bytes)}).${result.undo_lost_writes?` Undo is no longer available for ${result.undo_lost_writes} cleanup ${result.undo_lost_writes===1?"write":"writes"}.`:""}`);
    await load();
  }).then(()=>setConfirming(false));
  const retry=(entry:RetainedDisposal)=>void task.run("Finishing deletion…",null,async id=>{setHistory(await bridge.retryRetainedDisposal(id,entry.id));await load();});
  return <section className="maint-panel" aria-label="Preparation and cleanup originals">
    <h2>Preparation and cleanup originals</h2>
    <p className="cleanup-note">Tagging and cleanup keep a copy of each file they rewrite. These copies let a cleanup be undone and let an interrupted write recover. Disposing of them frees space; it never touches your music folders.</p>
    <div className="maint-summary" role="status"><strong>{storage?`${storage.original_files.toLocaleString()} ${storage.original_files===1?"original":"originals"} · ${bytes(storage.original_bytes)}`:"Measuring…"}</strong>
      <span>Filesystem snapshots or clones may keep the space in use after the files are deleted.</span></div>
    <div className="maint-controls">
      <label><input type="checkbox" checked={includeUndoable} disabled={!!task.busy} onChange={e=>{setIncludeUndoable(e.target.checked);setPreview(null);}}/>Also include originals that let me Undo a cleanup</label>
      <button className="primary maint-danger" disabled={!!task.busy||!storage||!storage.original_files} onClick={review}><HardDrive size={14}/>Review disposal of originals…</button>
    </div>
    {notice?<div role="status" className="maint-notice">{notice}</div>:null}
    {preview?<section className="maint-preview" aria-label="Originals disposal preview">
      <h2>Review disposal</h2>
      <p>{preview.files.length.toLocaleString()} retained originals ({bytes(preview.size_bytes)}) will be deleted from application storage.
        {preview.undo_lost_writes?` Undo will stop working for ${preview.undo_lost_writes} cleanup ${preview.undo_lost_writes===1?"write":"writes"}.`:" Undo is not affected."}</p>
      <ul className="maint-list" aria-label="Retained originals">{paged.visible.map(f=><li key={f.source+f.write_id+f.display_path}><span title={f.display_path}>{f.display_path}</span><small>{f.source==="tagging"?"Tagging":"Cleanup"}{f.undo_lost?" · backs Undo":""} · {bytes(f.size_bytes)}</small></li>)}</ul>
      <ShowMore {...paged}/>
      <div className="maint-controls"><button className="secondary" onClick={()=>setPreview(null)}>Cancel</button><button className="primary maint-danger" disabled={!!task.busy} onClick={()=>setConfirming(true)}>Dispose of originals…</button></div>
    </section>:null}
    {confirming&&preview?<ConfirmDialog title="Permanently dispose of originals?" confirmLabel="Dispose of originals" danger busy={!!task.busy} acknowledge={preview.undo_lost_writes?"I understand this cannot be undone and cleanup Undo will stop working for these writes.":"I understand this cannot be undone."} onConfirm={dispose} onCancel={()=>setConfirming(false)}>
      <p>{preview.files.length.toLocaleString()} retained originals ({bytes(preview.size_bytes)}) will be deleted. Your music library files are not touched.</p></ConfirmDialog>:null}
    <section className="maint-history" aria-label="Originals disposal history"><h3>Disposal history</h3>
      {!history.length?<p className="cleanup-note">Completed and interrupted disposals appear here.</p>:history.map(d=><article key={d.id} className="maint-history-row">
        <div><strong>{d.file_count.toLocaleString()} originals · {bytes(d.size_bytes)} reviewed</strong><small>{when(d.created)}{d.cleanup_pending?" · Deletion unfinished":""}</small>
          {d.error?<p role="alert" className={d.cleanup_pending?"maint-error-text":"maint-warn-text"}>{d.error}</p>:null}</div>
        <div className="maint-row-actions">{d.cleanup_pending?<button className="secondary" disabled={!!task.busy} onClick={()=>retry(d)}><RefreshCw size={13}/>Retry</button>:null}</div></article>)}
    </section>
  </section>;
}
