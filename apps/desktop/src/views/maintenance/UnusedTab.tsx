import {useCallback,useEffect,useMemo,useState} from "react";
import {FolderOpen,ScanSearch,Copy,Trash2,Undo2,RefreshCw} from "lucide-react";
import type {Bridge} from "../../types";
import {errorMessage} from "../../api";
import type {MaintenanceBridge} from "../../maintenance-api";
import type {UnusedRemovalHistory,UnusedRemovalPreview,UnusedScanReport,UnusedScanRequest} from "../../maintenance-types";
import type {MaintenanceTask} from "../../use-maintenance-task";
import {ConfirmDialog,RecoveryNote,ShowMore,StaleNotice,TaskStatus,bytes,needsRecovery,stateLabel,usePaged,when} from "./shared";

const defaultExtensions="mp3, m4a, flac, wav, aiff, ogg, opus, wma";
function parseExtensions(text:string){return [...new Set(text.split(/[\s,;]+/).map(v=>v.trim().replace(/^\./,"").toLowerCase()).filter(Boolean))];}

export default function UnusedTab({library,bridge,task,changeTick,onLibraryChanged}:{library:Bridge;bridge:MaintenanceBridge;task:MaintenanceTask;changeTick:number;onLibraryChanged:()=>void}){
  const [root,setRoot]=useState(""),[mode,setMode]=useState<UnusedScanRequest["extension_mode"]>("all"),[extensions,setExtensions]=useState(defaultExtensions);
  const [report,setReport]=useState<UnusedScanReport|null>(null),[selected,setSelected]=useState(new Set<string>());
  const [preview,setPreview]=useState<UnusedRemovalPreview|null>(null),[confirming,setConfirming]=useState(false),[exported,setExported]=useState<string|null>(null);
  const [history,setHistory]=useState<UnusedRemovalHistory[]>([]),[notice,setNotice]=useState<string|null>(null);
  const loadHistory=useCallback(()=>bridge.unusedRemovalHistory().then(setHistory).catch(e=>task.setError(errorMessage(e))),[bridge,task.setError]);
  useEffect(()=>{void loadHistory();},[loadHistory,changeTick]);
  const reportId=report?.id;
  useEffect(()=>{if(!reportId||!changeTick)return;let live=true;void task.run("Rechecking report…","unused",async id=>{const next=await bridge.unusedScan(id,reportId);if(live){setReport(next);if(next.stale)setPreview(null);}});return()=>{live=false;};
  // eslint-disable-next-line react-hooks/exhaustive-deps
  },[changeTick]);
  const choose=async()=>{try{const path=await library.chooseFolder();if(path){setRoot(path);}}catch(e){task.setError(errorMessage(e));}};
  const filtersInvalid=mode!=="all"&&!parseExtensions(extensions).length;
  const scan=()=>void task.run("Scanning folder…","unused",async id=>{
    const next=await bridge.scanUnused(id,{root,extension_mode:mode,extensions:mode==="all"?[]:parseExtensions(extensions)});
    setReport(next);setSelected(new Set());setPreview(null);setExported(null);setNotice(null);
  });
  const paged=usePaged(report?.files??[]);
  const toggle=(id:string)=>{setPreview(null);setSelected(old=>{const next=new Set(old);if(next.has(id))next.delete(id);else next.add(id);return next;});};
  const selectedBytes=useMemo(()=>(report?.files??[]).filter(f=>selected.has(f.id)).reduce((sum,f)=>sum+f.size_bytes,0),[report,selected]);
  const canRemove=!!report&&report.complete&&!report.stale;
  const makePreview=()=>report&&void task.run("Preparing removal preview…",null,async id=>{setPreview(await bridge.previewUnusedRemoval(id,report.id,[...selected]));setNotice(null);});
  const apply=()=>preview&&void task.run("Moving files to recovery storage…",null,async id=>{
    const result=await bridge.applyUnusedRemoval(id,preview.id);
    setConfirming(false);setPreview(null);setSelected(new Set());
    setNotice(`Removed ${result.files.length.toLocaleString()} files (${bytes(result.size_bytes)}). Originals are kept for Restore until you dispose of the backups.`);
    onLibraryChanged();await loadHistory();
  }).then(()=>setConfirming(false));
  const copyList=()=>report&&void task.run("Building path list…","unused",async id=>{
    const text=await bridge.exportUnused(id,report.id);setExported(text);
    try{await navigator.clipboard.writeText(text);}catch{/* The text stays visible for manual copying. */}
  });
  const restore=(entry:UnusedRemovalHistory)=>void task.run("Restoring files…",null,async id=>{
    const result=await bridge.restoreUnusedRemoval(id,entry.result.id);
    setNotice(`Restored ${result.files.length.toLocaleString()} files to their original locations.`);onLibraryChanged();await loadHistory();
  });
  const retry=(entry:UnusedRemovalHistory)=>void task.run("Retrying recovery…",null,async id=>{setHistory(await bridge.retryUnusedRecovery(id,entry.result.id));onLibraryChanged();});
  const siblings=preview?.files.filter(f=>f.role!=="selected").length??0;
  return <section className="maint-panel" aria-label="Unused files">
    <p className="cleanup-note">Find audio files in a folder that no library track uses. Scanning changes nothing. Removal moves files to recovery storage, where Restore can bring them back until you choose to dispose of the backups.</p>
    <div className="maint-controls maint-folder">
      <button className="secondary" disabled={!!task.busy} onClick={()=>void choose()}><FolderOpen size={14}/>Choose folder…</button>
      <span className="maint-root" title={root}>{root||"No folder chosen"}</span>
    </div>
    <div className="maint-controls">
      <label>Files to list<select aria-label="File filter" disabled={!!task.busy} value={mode} onChange={e=>setMode(e.target.value as UnusedScanRequest["extension_mode"])}><option value="all">All files</option><option value="include">Only these extensions</option><option value="exclude">All except these extensions</option></select></label>
      {mode!=="all"?<label>Extensions<input aria-label="Extensions" disabled={!!task.busy} value={extensions} onChange={e=>setExtensions(e.target.value)}/></label>:null}
      <button className="primary" disabled={!!task.busy||!root||filtersInvalid} onClick={scan}><ScanSearch size={14}/>Scan folder</button>
    </div>
    <TaskStatus task={task}/>
    {notice?<div role="status" className="maint-notice">{notice}</div>:null}
    {report?<>
      <StaleNotice stale={report.stale} reason={report.stale_reason} what="report"/>
      {!report.complete?<div role="alert" className="maint-stale"><strong>This scan is partial.</strong> Some folders could not be read, so files may be listed as unused incorrectly. Removal is disabled until a scan completes.</div>:null}
      {report.issues.length?<details className="maint-issues"><summary>{report.issues.length} scan {report.issues.length===1?"issue":"issues"}</summary><ul>{report.issues.map((issue,i)=><li key={i}><code>{issue.display_path}</code> {issue.message}</li>)}</ul></details>:null}
      <div className="maint-summary" role="status"><strong>{report.files.length.toLocaleString()} unused {report.files.length===1?"file":"files"} · {bytes(report.size_bytes)}</strong>
        <span>{selected.size?`${selected.size.toLocaleString()} selected · ${bytes(selectedBytes)}`:"Select files to review for removal."}</span></div>
      <div className="maint-controls">
        <button className="secondary" disabled={!!task.busy||!report.files.length} onClick={()=>{setPreview(null);setSelected(old=>old.size===report.files.length?new Set():new Set(report.files.map(f=>f.id)));}}>{selected.size===report.files.length&&selected.size?"Clear selection":"Select all"}</button>
        <button className="secondary" disabled={!!task.busy||!report.files.length} onClick={copyList}><Copy size={14}/>Copy path list</button>
        <button className="primary" disabled={!!task.busy||!canRemove||!selected.size} onClick={makePreview}><Trash2 size={14}/>Review removal…</button>
      </div>
      {exported!==null?<details className="maint-export" open><summary>Path list (copied to clipboard)</summary><textarea readOnly aria-label="Unused file path list" value={exported}/></details>:null}
      {report.files.length?<table className="maint-table"><thead><tr><th/><th>File</th><th>Size</th></tr></thead><tbody>
        {paged.visible.map(f=><tr key={f.id}><td><input type="checkbox" aria-label={"Select "+f.display_path} disabled={!!task.busy||!canRemove} checked={selected.has(f.id)} onChange={()=>toggle(f.id)}/></td><td title={f.display_path}>{f.display_path}</td><td>{bytes(f.size_bytes)}</td></tr>)}
      </tbody></table>:<p className="cleanup-note">Every file in this folder is used by a library track.</p>}
      <ShowMore {...paged}/>
    </>:null}
    {preview?<section className="maint-preview" aria-label="Removal preview">
      <h2>Review removal</h2>
      <p>{preview.files.length.toLocaleString()} files ({bytes(preview.size_bytes)}) will move out of your music folders{siblings?`, including ${siblings} related sidecar or folder-metadata ${siblings===1?"file":"files"}`:""}. Track metadata, cues and playlists are not changed.</p>
      <ul className="maint-list" aria-label="Files to remove">{preview.files.map(f=><li key={f.id}><span title={f.display_path}>{f.display_path}</span><small>{f.role==="selected"?"Selected":f.role==="sibling"?"Sibling file":"Folder metadata"} · {bytes(f.size_bytes)}</small></li>)}</ul>
      {preview.directories.length?<><h3>Empty folders to remove</h3><ul className="maint-list" aria-label="Folders to remove">{preview.directories.map(d=><li key={d}><span title={d}>{d}</span></li>)}</ul></>:null}
      <div className="maint-controls"><button className="secondary" onClick={()=>setPreview(null)}>Cancel</button><button className="primary maint-danger" disabled={!!task.busy} onClick={()=>setConfirming(true)}>Remove {preview.files.length.toLocaleString()} files…</button></div>
    </section>:null}
    {confirming&&preview?<ConfirmDialog title="Remove these files?" confirmLabel={`Remove ${preview.files.length.toLocaleString()} files`} danger busy={!!task.busy} onConfirm={apply} onCancel={()=>setConfirming(false)}>
      <p>{preview.files.length.toLocaleString()} files ({bytes(preview.size_bytes)}) will be moved to recovery storage. Use Restore in the history below to bring them back to their original locations.</p>
      <TaskStatus task={task}/>
    </ConfirmDialog>:null}
    <section className="maint-history" aria-label="Removal history"><h2>Removal history</h2>
      {!history.length?<p className="cleanup-note">Removed files and restore results appear here.</p>:history.map(h=><article key={h.result.id} className="maint-history-row">
        <div><strong>{h.result.undo_of?"Restore":"Removal"} · {h.result.files.length.toLocaleString()} files · {bytes(h.result.size_bytes)}</strong>
          <small>{stateLabel(h.state)}{h.undone_by?" · Restored":""}</small>
          <RecoveryNote state={h.state} error={h.error} cleanupPending={h.cleanup_pending}/></div>
        <div className="maint-row-actions">
          {needsRecovery(h.state,h.cleanup_pending)?<button className="secondary" disabled={!!task.busy} onClick={()=>retry(h)}><RefreshCw size={13}/>Retry recovery</button>:null}
          {!h.result.undo_of&&h.state==="committed"&&!h.undone_by?<button className="secondary" disabled={!!task.busy||!h.restore_available} title={h.restore_available?undefined:"Restore is no longer available"} onClick={()=>restore(h)}><Undo2 size={13}/>Restore</button>:null}
        </div></article>)}
    </section>
  </section>;
}
