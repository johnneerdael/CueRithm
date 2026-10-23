import {useEffect,useState} from "react";
import {Archive,FolderOpen,ShieldCheck} from "lucide-react";
import type {LibraryBackupBridge,LibraryBackupPreview,LibraryRestoreRequest} from "../backup-types";
import {nativeLibraryBackup} from "../backup-api";
import {errorMessage} from "../api";
import "./LibraryBackup.css";
import PortableBackup from "./PortableBackup";
import type {PortableBackupBridge} from "../portable-backup-types";

export default function LibraryBackup({bridge=nativeLibraryBackup,portableBridge,onChanged}:{bridge?:LibraryBackupBridge;portableBridge?:PortableBackupBridge;onChanged?:()=>void}){return <div className="library-backup-workspace"><PortableBackup bridge={portableBridge} onChanged={onChanged}/><details className="installation-backup"><summary>Same-installation recovery snapshots</summary><InstallationBackup bridge={bridge}/></details></div>;}

export function InstallationBackup({bridge=nativeLibraryBackup}:{bridge?:LibraryBackupBridge}){
 const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null);
 const [preview,setPreview]=useState<LibraryBackupPreview|null>(null),[created,setCreated]=useState<LibraryBackupPreview|null>(null);
 const [confirmed,setConfirmed]=useState(false),[pending,setPending]=useState<LibraryRestoreRequest|null>(null);
 useEffect(()=>{let active=true;bridge.pending().then(value=>{if(active)setPending(value);}).catch(e=>{if(active)setError(errorMessage(e));});return()=>{active=false;};},[bridge]);
 async function choose(create:boolean){
  setBusy(true);setError(null);setConfirmed(false);setPreview(null);
  try{const folder=await bridge.chooseFolder(create?"Choose where to save the backup":"Select a CueRithm backup folder");if(!folder)return;
   const result=await (create?bridge.create(folder):bridge.inspect(folder));
   if(create)setCreated(result);else setPreview(result);
  }catch(e){setError(errorMessage(e));}finally{setBusy(false);}
 }
 return <section className="library-backup-view" aria-label="Library backup and restore">
  <div className="view-title"><div><h1>Library backup</h1><p>Keep a dated copy of your preparation work.</p></div></div>
  {error?<div role="alert" className="error-banner">{error}</div>:null}
  <div className="backup-scope"><ShieldCheck size={24}/><div><h2>Your preparation, saved together</h2><p>Back up playlists, cues, analysis and library settings. Music files are excluded. Keep a separate backup of your music.</p><p>Restore requires this library installation and CueRithm version, with unchanged music, analysis models and recovery files. Restoring the library cannot undo edits to music tags or DJ software.</p></div></div>
  <div className="backup-actions"><button className="primary" disabled={busy||!!pending} onClick={()=>void choose(true)}><Archive size={16}/>Create backup</button><button className="secondary" disabled={busy||!!pending} onClick={()=>void choose(false)}><FolderOpen size={16}/>Inspect backup folder</button>{busy?<span role="status">Checking library and file consistency…</span>:null}</div>
  {created?<div className="backup-result" role="status"><h2>Backup saved</h2><p>{new Date(created.created).toLocaleString()} · {created.tracks} tracks</p><p className="backup-path">{created.path}</p><p>The private location manifest stays inside this folder. Keep both files together.</p></div>:null}
  {preview&&!pending?<section className="backup-preview" aria-label="Restore preview"><h2>Restore preview</h2><p>{new Date(preview.created).toLocaleString()}</p><strong>{preview.tracks} tracks</strong><p>{preview.missing_sources?`${preview.missing_sources} source files were unavailable when saved and remain unavailable.`:"Music locations and retained recovery files match this backup."}</p><p className="backup-path">{preview.path}</p><p>Your current library is kept as a pre-restore backup. This restore is applied before the library opens on your next launch.</p><label className="backup-confirm"><input type="checkbox" checked={confirmed} disabled={busy} onChange={e=>setConfirmed(e.target.checked)}/><span>Replace my current preparation state with this backup on the next launch.</span></label><button className="primary" disabled={busy||!confirmed} onClick={async()=>{setBusy(true);setError(null);try{setPending(await bridge.stage(preview.path,preview.database_sha256));setPreview(null);setConfirmed(false);}catch(e){setError(errorMessage(e));setPreview(null);setConfirmed(false);}finally{setBusy(false);}}}>Restore on next launch</button></section>:null}
  {pending?<div className="backup-result" role="status"><h2>{pending.blocked_reason?"Restore blocked":"Restore scheduled"}</h2>{pending.blocked_reason?<><p role="alert" className="error-banner">{pending.blocked_reason}</p><p>Your current library was left unchanged. Fix the reported file consistency problem and retry, or cancel this restore.</p><button className="secondary" disabled={busy} onClick={async()=>{setBusy(true);setError(null);try{setPending(await bridge.retry());}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}}>Retry on next launch</button></>:<><p>Close and reopen CueRithm to apply the restore. File consistency is checked again before anything changes.</p><p>Finish any running work before closing. The restore will stop if music or recovery files change.</p></>}<button className="secondary" disabled={busy} onClick={async()=>{setBusy(true);setError(null);try{await bridge.cancel();setPending(null);}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}}>Cancel scheduled restore</button></div>:null}
 </section>;
}
