import {useCallback,useEffect,useMemo,useState} from "react";
import {ChevronLeft,Download,Eye,RefreshCw,Upload} from "lucide-react";
import {errorMessage} from "../api";
import type {MaintenanceBridge} from "../maintenance-api";
import type {RekordboxBridge} from "../rekordbox-api";
import type {ApplyOutcome,FieldDecision,NativeSyncPrepared,NativeSyncRun,PlaylistAction,RekordboxPlaylist,RekordboxPreflight} from "../rekordbox-types";
import {useMaintenanceTask} from "../use-maintenance-task";
import {ConfirmDialog,TaskStatus,when} from "./maintenance/shared";

const decisionText:Record<FieldDecision,string>={
  unchanged:"Unchanged",converged:"Already equal",push_local:"Write to rekordbox",pull_remote:"rekordbox is newer",
  conflict:"Conflict — both changed",needs_owner:"Needs an owner",empty_local:"Cuerithm has no value",
};
const playlistText:Record<PlaylistAction,string>={create:"Create",update:"Update",unchanged:"Unchanged",remove:"Remove",skip_existing:"Left alone — already in rekordbox",skip_changed:"Left alone — changed in rekordbox"};
const fieldNames:Record<string,string>={tag_ids:"MyTags",mix_name:"Mix",release_year:"Year",release_date:"Release date",track_number:"Track number",disc_number:"Disc number",cues:"Cues",bpm:"BPM"};
const fieldName=(f:string)=>fieldNames[f]??f.charAt(0).toUpperCase()+f.slice(1).replace(/_/g," ");
function show(value:unknown){
  if(value===null||value===undefined||value==="")return "—";
  if(Array.isArray(value)){
    if(!value.length)return "—";
    // MyTags are "category, tag" strings; cues are rows.
    if(value.every(v=>typeof v==="string"))return value.map(v=>String(v).replace("\u001f"," / ")).join(", ");
    return `${value.length} cue${value.length===1?"":"s"}`;
  }
  return String(value);
}

/**
 * Synchronize with an installed rekordbox library: check it, link tracks, preview exactly what would be written,
 * then publish with a verified backup. Cuerithm only ever writes fields it owns and has changed.
 */
export default function Rekordbox({rekordbox,maintenance,onLibraryChanged,onBack}:{rekordbox:RekordboxBridge;maintenance:MaintenanceBridge;onLibraryChanged:()=>void;onBack:()=>void}){
  const task=useMaintenanceTask(maintenance);
  const [preflight,setPreflight]=useState<RekordboxPreflight|null>(null),[playlists,setPlaylists]=useState<RekordboxPlaylist[]|null>(null);
  const [scope,setScope]=useState("all"),[prepared,setPrepared]=useState<NativeSyncPrepared|null>(null),[history,setHistory]=useState<NativeSyncRun[]>([]);
  const [outcome,setOutcome]=useState<ApplyOutcome|null>(null),[notice,setNotice]=useState<string|null>(null),[confirming,setConfirming]=useState(false);
  const load=useCallback(async()=>{
    try{const [p,h]=await Promise.all([rekordbox.preflight(),rekordbox.history()]);setPreflight(p);setHistory(h);}
    catch(e){task.setError(errorMessage(e));}
  },[rekordbox,task.setError]);
  useEffect(()=>{void load();},[load]);
  const ready=!!preflight?.key_ready&&!!preflight.database_path&&preflight.database_state==="present";
  const linked=preflight?.targets.reduce((n,t)=>n+t.tracks,0)??0;
  const loadPlaylists=()=>void task.run("Reading rekordbox playlists…",null,async id=>{setPlaylists(await rekordbox.playlists(id));});
  const importTracks=()=>void task.run("Linking tracks from rekordbox…",null,async id=>{
    const report=await rekordbox.importTracks(id,scope==="all"?{kind:"all"}:{kind:"playlist",id:scope});
    setNotice(`Linked ${Object.values(report.statuses).reduce((a,b)=>a+b,0).toLocaleString()} tracks: ${report.tracks_created.toLocaleString()} new tracks are in Incoming for preparation.`);
    setPrepared(null);onLibraryChanged();await load();
  });
  const preview=()=>void task.run("Reading rekordbox and comparing…",null,async id=>{setPrepared(await rekordbox.prepare(id));setOutcome(null);setNotice(null);});
  const apply=()=>void task.run("Writing to a copy, then publishing with a backup…",null,async id=>{
    const result=await rekordbox.apply(id);
    setOutcome(result);setPrepared(null);
    if(result.run?.state==="committed")setNotice(`Wrote ${result.run.tracks.length.toLocaleString()} tracks to rekordbox. A verified backup was made first.`);
    else if(result.run)setNotice("Nothing needed writing.");
    await load();
  }).then(()=>setConfirming(false));
  const playlistWrites=prepared?.playlists.items.filter(i=>i.action==="create"||i.action==="update"||i.action==="remove").length??0;
  const plannedFields=(prepared?.plan.tracks.reduce((n,t)=>n+t.fields.length,0)??0)+playlistWrites;
  const attention=useMemo(()=>prepared?.preview.tracks.filter(t=>t.blocked||t.fields.some(f=>f.decision!=="pull_remote"&&f.decision!=="converged")).length??0,[prepared]);
  return <section className="rekordbox-view cleanup-view">
    <div className="view-title"><div><button className="back-link" onClick={onBack}><ChevronLeft size={15}/>Library</button><div className="eyebrow">LIBRARY TOOLS</div><h1>rekordbox</h1><p>Send tracks you have changed in CueRithm to your rekordbox library. Nothing is written until you review it, and a verified backup comes first.</p></div></div>
    <div className="cleanup-body maint-body">
      <TaskStatus task={task}/>
      {notice?<div role="status" className="maint-notice">{notice}</div>:null}
      <section className="maint-panel" aria-label="Preflight"><h2>Preflight</h2>
        {!preflight?<p className="cleanup-note">Checking…</p>:<ul className="maint-list rb-checks" aria-label="Preflight checks">
          <li><span>rekordbox installed</span><small>{preflight.installations.length?`Version ${preflight.installations[0].version}`:preflight.platform_supported?"Not found":"Not supported on this platform"}</small></li>
          <li><span>Library found</span><small title={preflight.database_path??undefined}>{preflight.database_path?`${preflight.database_state}`:"Not found"}</small></li>
          <li><span>Library can be opened</span><small>{preflight.key_ready?"Ready":preflight.key_error??"Not ready"}</small></li>
          <li><span>Writing is verified for this library</span><small>{preflight.write_supported?`Yes (${preflight.write_profiles.join(", ")})`:preflight.schema_fingerprint?`No — schema ${preflight.schema_fingerprint} is not verified. Previewing still works.`:"Not checked"}</small></li>
          <li><span>Linked tracks</span><small>{linked.toLocaleString()}</small></li>
        </ul>}
        <div className="maint-controls"><button className="secondary" disabled={!!task.busy} onClick={()=>void load()}><RefreshCw size={14}/>Check again</button></div>
        <p className="cleanup-note">Before applying, close rekordbox and its background agent. If either has the library open, the write stops and nothing changes.</p>
      </section>
      <section className="maint-panel" aria-label="Link tracks"><h2>Link tracks</h2>
        <p className="cleanup-note">Linking reads your rekordbox library and brings its tracks into CueRithm. New tracks go to Incoming and are prepared like any others. Link tracks before you preview a sync.</p>
        <div className="maint-controls">
          <label>Which tracks<select aria-label="Tracks to link" disabled={!!task.busy||!ready} value={scope} onChange={e=>setScope(e.target.value)} onFocus={()=>{if(!playlists)loadPlaylists();}}>
            <option value="all">Every track</option>{(playlists??[]).filter(p=>!p.is_folder).map(p=><option key={p.id} value={p.id}>{p.name} ({p.tracks.toLocaleString()})</option>)}</select></label>
          <button className="primary" disabled={!!task.busy||!ready} onClick={importTracks}><Download size={14}/>Link tracks</button>
        </div>
      </section>
      <section className="maint-panel" aria-label="Preview"><h2>Preview</h2>
        <div className="maint-controls"><button className="primary" disabled={!!task.busy||!ready||!linked} onClick={preview}><Eye size={14}/>Preview changes</button>
          {prepared&&preflight?.write_supported?<button className="primary maint-danger" disabled={!!task.busy||!plannedFields} onClick={()=>setConfirming(true)}><Upload size={14}/>Apply {plannedFields.toLocaleString()} {plannedFields===1?"change":"changes"}…</button>:null}</div>
        {!linked?<p className="cleanup-note">Link tracks first.</p>:null}
        {prepared?<>
          <div className="maint-summary" role="status"><strong>{prepared.plan.tracks.length.toLocaleString()} {prepared.plan.tracks.length===1?"track":"tracks"} and {playlistWrites.toLocaleString()} {playlistWrites===1?"playlist":"playlists"} to write</strong>
            <span>{prepared.plan.skipped.length.toLocaleString()} need a decision · {prepared.plan.pull_available.toLocaleString()} where rekordbox is newer (never overwritten) · {prepared.plan.blocked.length.toLocaleString()} blocked</span></div>
          {!preflight?.write_supported?<div role="alert" className="maint-stale">This library's schema is not verified for writing, so changes can be previewed but not applied.</div>:null}
          {prepared.playlists.items.some(i=>i.action!=="unchanged")?<ul className="maint-list" aria-label="Playlists to review">{prepared.playlists.items.filter(i=>i.action!=="unchanged").map((i,n)=><li key={n}><span title={i.path.join(" / ")}>{i.kind==="folder"?"Folder: ":i.kind==="smartlist"?"Smart list: ":""}{i.path.join(" / ")}</span><small>{playlistText[i.action]}{i.kind==="smartlist"?" · rekordbox keeps it up to date":""}{i.materialized?" · written as a playlist because rekordbox cannot express a rule":""}{(i.action==="create"||i.action==="update")&&i.kind==="playlist"?` · ${i.tracks.toLocaleString()} tracks`:""}{i.unlinked?` · ${i.unlinked.toLocaleString()} not linked to rekordbox`:""}</small></li>)}</ul>:null}
          {attention===0&&!prepared.plan.tracks.length&&!playlistWrites?<p className="cleanup-note">rekordbox and CueRithm already agree.</p>:null}
          <ul className="rb-tracks" aria-label="Tracks to review">{prepared.preview.tracks.map(t=><li key={t.content_id}>
            <strong>Track {t.content_id}</strong>{t.blocked?<span className="maint-state bad">Blocked: {t.blocked.replace(/_/g," ")}</span>:null}
            <table className="maint-table"><thead><tr><th>Field</th><th>CueRithm</th><th>rekordbox</th><th>Result</th></tr></thead><tbody>
              {t.fields.filter(f=>f.decision!=="converged").map(f=><tr key={f.field}><td>{fieldName(f.field)}</td><td title={show(f.local)}>{show(f.local)}</td><td title={show(f.remote)}>{show(f.remote)}</td>
                <td><span className={"maint-state "+(f.decision==="push_local"?"ok":f.decision==="pull_remote"?"":"warn")}>{decisionText[f.decision]}</span></td></tr>)}</tbody></table></li>)}</ul>
        </>:null}
        {outcome?.conflicts.length?<div role="alert" className="maint-stale"><strong>Nothing was written.</strong> rekordbox changed these after they were read. Preview again to see the current state.
          <ul>{outcome.conflicts.map((c,i)=><li key={i}><code>{c.subject}</code> {c.reason}</li>)}</ul></div>:null}
      </section>
      <section className="maint-history" aria-label="Sync history"><h2>History</h2>
        {!history.length?<p className="cleanup-note">Completed syncs appear here with the backup that was made first.</p>:history.map(h=><article key={h.id} className="maint-history-row">
          <div><strong>{h.state==="committed"?`Wrote ${h.tracks.length.toLocaleString()} ${h.tracks.length===1?"track":"tracks"}${h.playlists.items.some(i=>i.action==="create"||i.action==="update"||i.action==="remove")?` and ${h.playlists.items.filter(i=>i.action==="create"||i.action==="update"||i.action==="remove").length} playlists`:""}`:"Nothing to write"}</strong><small>{when(h.created)}{h.skipped?` · ${h.skipped} left for a decision`:""}</small>
            {h.backup?<small title={h.backup}>Backup: {h.backup}</small>:null}</div></article>)}
      </section>
    </div>
    {confirming&&prepared?<ConfirmDialog title="Write to your rekordbox library?" confirmLabel={`Write ${plannedFields.toLocaleString()} ${plannedFields===1?"change":"changes"}`} danger busy={!!task.busy}
      acknowledge="rekordbox and its background agent are closed, and I want these changes written." onConfirm={apply} onCancel={()=>setConfirming(false)}>
      <p>{prepared.plan.tracks.length.toLocaleString()} {prepared.plan.tracks.length===1?"track":"tracks"} and {playlistWrites.toLocaleString()} {playlistWrites===1?"playlist":"playlists"} will be updated in your rekordbox library. CueRithm writes to a copy first, makes a verified backup of the library as it is now, publishes the copy, and checks the result. If anything fails, the backup is restored automatically.</p>
      <p>Tracks that rekordbox changed since they were read are never overwritten.</p><TaskStatus task={task}/></ConfirmDialog>:null}
  </section>;
}
