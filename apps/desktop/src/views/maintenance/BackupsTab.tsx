import {useCallback,useEffect,useState} from "react";
import {HardDrive,RefreshCw} from "lucide-react";
import {errorMessage} from "../../api";
import type {Bridge} from "../../types";
import RetainedOriginals from "./RetainedOriginals";
import type {MaintenanceBridge} from "../../maintenance-api";
import type {UnusedBackupDisposal,UnusedBackupPreview,UnusedRemovalHistory} from "../../maintenance-types";
import type {MaintenanceTask} from "../../use-maintenance-task";
import {ConfirmDialog,RecoveryNote,TaskStatus,bytes,needsRecovery,stateLabel} from "./shared";

/**
 * Retained originals from reviewed file removals occupy application storage until disposed.
 * Disposal is irreversible: it removes Restore for the selected removals.
 */
export default function BackupsTab({library,bridge,task,changeTick}:{library:Bridge;bridge:MaintenanceBridge;task:MaintenanceTask;changeTick:number}){
  const [removals,setRemovals]=useState<UnusedRemovalHistory[]>([]),[disposals,setDisposals]=useState<UnusedBackupDisposal[]>([]);
  const [selected,setSelected]=useState(new Set<string>()),[preview,setPreview]=useState<UnusedBackupPreview|null>(null),[confirming,setConfirming]=useState(false),[notice,setNotice]=useState<string|null>(null);
  const load=useCallback(async()=>{
    try{const [r,d]=await Promise.all([bridge.unusedRemovalHistory(),bridge.backupDisposalHistory()]);setRemovals(r);setDisposals(d);}
    catch(e){task.setError(errorMessage(e));}
  },[bridge,task.setError]);
  useEffect(()=>{setPreview(null);void load();},[load,changeTick]);
  // Disposal selects original removals, not the reverse restore journals. A restored removal still holds its
  // recovery copies until they are disposed of, so it stays listed; one whose copies are gone does not.
  const originals=removals.filter(h=>!h.result.undo_of&&["committed","undone","rolled_back"].includes(h.state)&&!h.backups_disposed&&!h.cleanup_pending);
  const toggle=(id:string)=>{setPreview(null);setSelected(old=>{const next=new Set(old);if(next.has(id))next.delete(id);else next.add(id);return next;});};
  const review=()=>void task.run("Measuring retained backups…",null,async id=>{setPreview(await bridge.previewBackupDisposal(id,[...selected]));setNotice(null);});
  const dispose=()=>preview&&void task.run("Disposing of backups…",null,async id=>{
    const result=await bridge.disposeBackups(id,preview.id);
    setPreview(null);setSelected(new Set());
    setNotice(result.cleanup_pending?"Disposal is recorded but cleanup is unfinished. Retry it from the history below.":`Disposed of ${result.file_count.toLocaleString()} backup files (${bytes(result.size_bytes)} reviewed). Restore is no longer available for those removals.`);
    await load();
  }).then(()=>setConfirming(false));
  const retry=(entry:UnusedBackupDisposal)=>void task.run("Retrying disposal…",null,async id=>{setDisposals(await bridge.retryBackupDisposal(id,entry.id));});
  return <><section className="maint-panel" aria-label="Retained backups">
    <p className="cleanup-note">Files you remove are kept in application storage so Restore can bring them back. Disposing of those backups frees space but permanently removes Restore. Music files are never deleted here, and nothing is disposed of automatically.</p>
    <TaskStatus task={task}/>
    {notice?<div role="status" className="maint-notice">{notice}</div>:null}
    <h2>Removed files</h2>
    {!originals.length?<p className="cleanup-note">No removal backups are being retained.</p>:<table className="maint-table"><thead><tr><th/><th>Removal</th><th>Files</th><th>Size</th><th>Restore</th></tr></thead><tbody>
      {originals.map(h=><tr key={h.result.id}><td><input type="checkbox" aria-label={`Select removal ${h.result.id.slice(0,8)}`} disabled={!!task.busy} checked={selected.has(h.result.id)} onChange={()=>toggle(h.result.id)}/></td>
        <td>Removal <small>{h.result.id.slice(0,8)}</small></td><td>{h.result.files.length.toLocaleString()}</td><td>{bytes(h.result.size_bytes)}</td>
        <td>{h.undone_by?"Restored":h.restore_available?"Available":"Unavailable"}</td></tr>)}
    </tbody></table>}
    <div className="maint-controls"><button className="primary maint-danger" disabled={!!task.busy||!selected.size} onClick={review}><HardDrive size={14}/>Review disposal of backups…</button></div>
    {preview?<section className="maint-preview" aria-label="Disposal preview">
      <h2>Review disposal</h2>
      <p>{preview.files.length.toLocaleString()} backup files ({bytes(preview.size_bytes)}) will be deleted from application storage.
        {preview.restores_lost.length?` Restore will be lost for ${preview.restores_lost.length} ${preview.restores_lost.length===1?"removal":"removals"}.`:""} Filesystem snapshots or clones may keep the physical space in use after the logical files are deleted.</p>
      <ul className="maint-list" aria-label="Backup files">{preview.files.map(f=><li key={f.operation_id+":"+f.file_id}><span title={f.display_source_path}>{f.display_source_path}</span><small>{bytes(f.size_bytes)}</small></li>)}</ul>
      <div className="maint-controls"><button className="secondary" onClick={()=>setPreview(null)}>Cancel</button><button className="primary maint-danger" disabled={!!task.busy} onClick={()=>setConfirming(true)}>Dispose of backups…</button></div>
    </section>:null}
    {confirming&&preview?<ConfirmDialog title="Permanently dispose of backups?" confirmLabel="Dispose of backups" danger busy={!!task.busy} acknowledge="I understand this cannot be undone and removes Restore for these removals." onConfirm={dispose} onCancel={()=>setConfirming(false)}>
      <p>{preview.files.length.toLocaleString()} backup files ({bytes(preview.size_bytes)}) will be deleted. Your music library is not touched.</p>
      <TaskStatus task={task}/>
    </ConfirmDialog>:null}
    <section className="maint-history" aria-label="Disposal history"><h2>Disposal history</h2>
      {!disposals.length?<p className="cleanup-note">Completed and interrupted disposals appear here.</p>:disposals.map(d=><article key={d.id} className="maint-history-row">
        <div><strong>{d.file_count.toLocaleString()} backup files · {bytes(d.size_bytes)} reviewed</strong><small>{stateLabel(d.state)}</small><RecoveryNote state={d.state} error={d.error} cleanupPending={d.cleanup_pending}/></div>
        <div className="maint-row-actions">{needsRecovery(d.state,d.cleanup_pending)?<button className="secondary" disabled={!!task.busy} onClick={()=>retry(d)}><RefreshCw size={13}/>Retry</button>:null}</div></article>)}
    </section>
  </section>
  <RetainedOriginals library={library} bridge={bridge} task={task} changeTick={changeTick}/></>;
}
