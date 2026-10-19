import {useEffect,useMemo,useState} from "react";
import {ScanSearch,GitMerge,Lock,LockOpen,EyeOff,Eye} from "lucide-react";
import {errorMessage} from "../../api";
import type {Bridge} from "../../types";
import type {PlaylistNode} from "../../playlist-types";
import type {MaintenanceBridge} from "../../maintenance-api";
import type {DuplicateMergePreview,DuplicateReview,DuplicateReviewEdit,DuplicateScanReport,DuplicateScanRequest,GroupEdit,Preference,ReviewGroup,ReviewTrack} from "../../maintenance-types";
import type {MaintenanceTask} from "../../use-maintenance-task";
import {ConfirmDialog,ShowMore,StaleNotice,TaskStatus,bytes,usePaged} from "./shared";
import {stateLabels} from "./BrokenTab";
import DuplicateExtras from "./DuplicateExtras";

const tolerances:[DuplicateScanRequest["tolerance"],string][]=[["None","Exact text"],["Low","Low tolerance"],["Medium","Medium tolerance"],["High","High tolerance"],["High_IgnoreParentheses","High, ignore parentheses"]];
const scopes:[DuplicateScanRequest["scope"],string][]=[["full_library","Whole library"],["selected_playlists","Selected playlists only"],["full_library_filtered","Whole library, groups touching selected playlists"]];
const preferenceKinds:[Preference["kind"],string][]=[["most_cuepoints","Most cue points"],["highest_bitrate","Highest bitrate"],["folder","Located in folder"],["title_contains","Title contains"],["extra1_contains","Extra field contains"],["extra1_not_contains","Extra field does not contain"]];
const clock=(seconds:number)=>`${Math.floor(seconds/60)}:${String(Math.round(seconds%60)).padStart(2,"0")}`;
const label=(t:ReviewTrack)=>`${t.candidate.artist||"Unknown artist"} – ${t.candidate.title||"Untitled"}`;
const canKeep=(group:ReviewGroup,t:ReviewTrack)=>t.candidate.present&&!group.ignored.includes(t.candidate.id);
const mergeable=(g:ReviewGroup)=>g.keeper!==null&&!g.ignored.includes(g.keeper)&&g.tracks.some(t=>t.candidate.id===g.keeper&&t.candidate.present);

function GroupCard({group,disabled,selected,onSelect,onEdit}:{group:ReviewGroup;disabled:boolean;selected:boolean;onSelect:(on:boolean)=>void;onEdit:(edit:GroupEdit)=>void}){
  const names=`${group.tracks[0]?label(group.tracks[0]):"Group"}`;
  return <article className="maint-group" aria-label={"Duplicate group "+names}>
    <header><label><input type="checkbox" disabled={disabled||!mergeable(group)} checked={selected} onChange={e=>onSelect(e.target.checked)} aria-label={"Include group "+names}/><strong>{names}</strong></label>
      <span className="maint-state">{group.exact_audio?"Identical audio":"Artist/title match"}</span>
      <span className="maint-state">{group.state==="locked"?"Locked":group.state==="copy_cues"?"Copy tags and cues only":"Merge"}</span>
      {group.keeper&&group.ignored.includes(group.keeper)?<span className="maint-state warn">Keeper is ignored</span>:null}
      <div className="maint-row-actions">
        {group.state==="locked"?<><button className="secondary" disabled={disabled} onClick={()=>onEdit({kind:"unlock",force:false})}><LockOpen size={13}/>Unlock</button>{group.tags_from?null:<button className="secondary" disabled={disabled} onClick={()=>onEdit({kind:"unlock",force:true})}>Force unlock</button>}</>
          :<button className="secondary" disabled={disabled} onClick={()=>onEdit({kind:"lock"})}><Lock size={13}/>Lock</button>}
        {group.state==="locked"&&group.tags_from?<button className="secondary" disabled={disabled} onClick={()=>onEdit({kind:"unlock",force:true})}>Force unlock</button>:null}
      </div></header>
    <table className="maint-table"><thead><tr><th>Keep</th><th>Track</th><th>Location</th><th>Length</th><th>Bitrate</th><th>Cues</th><th>Tags and cues from</th><th/></tr></thead><tbody>
      {group.tracks.map(t=>{const c=t.candidate,ignored=group.ignored.includes(c.id);
        return <tr key={c.id} className={ignored?"ignored":undefined}>
          <td><input type="radio" name={"keeper-"+group.id} aria-label={"Keep "+label(t)} disabled={disabled||!canKeep(group,t)||group.state==="locked"} checked={group.keeper===c.id} onChange={()=>onEdit({kind:"select",track_id:c.id})}/></td>
          <td><strong>{label(t)}</strong>{!c.present?<span className="maint-state bad">Missing</span>:null}{ignored?<span className="maint-state warn">Ignored</span>:null}</td>
          <td title={c.location}>{c.location}</td><td>{clock(c.duration)}</td><td>{c.bitrate?Math.round(c.bitrate/1000)+" kbps":"—"}</td><td>{c.cue_count}</td>
          <td><input type="radio" name={"donor-"+group.id} aria-label={"Take tags and cues from "+label(t)} disabled={disabled||!c.present||ignored||group.state==="locked"||group.keeper===c.id} checked={group.tags_from===c.id} onChange={()=>onEdit({kind:"use_tags",track_id:c.id})}/>
            {group.tags_from===c.id?<button className="back-link" disabled={disabled} onClick={()=>onEdit({kind:"use_tags",track_id:null})}>Clear</button>:null}</td>
          <td><button className="icon-button" aria-label={(ignored?"Stop ignoring ":"Ignore ")+label(t)} title={ignored?"Include in duplicate scans again":"Leave out of duplicate scans"} disabled={disabled} onClick={()=>onEdit({kind:"toggle_ignore",track_id:c.id})}>{ignored?<Eye size={14}/>:<EyeOff size={14}/>}</button></td></tr>;})}
    </tbody></table></article>;
}

export default function DuplicatesTab({library,bridge,task,changeTick,onLibraryChanged}:{library:Bridge;bridge:MaintenanceBridge;task:MaintenanceTask;changeTick:number;onLibraryChanged:()=>void}){
  const [scope,setScope]=useState<DuplicateScanRequest["scope"]>("full_library"),[tolerance,setTolerance]=useState<DuplicateScanRequest["tolerance"]>("None"),[playlistIds,setPlaylistIds]=useState<string[]>([]);
  const [playlists,setPlaylists]=useState<PlaylistNode[]>([]),[scan,setScan]=useState<DuplicateScanReport|null>(null),[review,setReview]=useState<DuplicateReview|null>(null);
  const [chosen,setChosen]=useState(new Set<string>()),[preview,setPreview]=useState<DuplicateMergePreview|null>(null),[confirming,setConfirming]=useState(false),[notice,setNotice]=useState<string|null>(null);
  const [pref,setPref]=useState<Preference["kind"]>("most_cuepoints"),[prefValue,setPrefValue]=useState(""),[override,setOverride]=useState(false),[bestCues,setBestCues]=useState(false);
  useEffect(()=>{let live=true;void library.getPlaylists().then(w=>{if(live)setPlaylists(w.tree.nodes.filter(n=>!n.deleted&&n.kind!=="folder"));}).catch(()=>{});return()=>{live=false;};},[library]);
  const needsPlaylists=scope!=="full_library",invalid=needsPlaylists&&!playlistIds.length;
  const runScan=()=>void task.run("Scanning for duplicates…","duplicates",async id=>{
    const next=await bridge.scanDuplicates(id,{scope,playlist_ids:needsPlaylists?playlistIds:[],tolerance,ignored_paths:[]});
    setScan(next);setReview(null);setPreview(null);setChosen(new Set());setNotice(null);
  });
  const begin=()=>scan&&void task.run("Opening review…",null,async id=>{setReview(await bridge.beginReview(id,scan.id));setChosen(new Set());setPreview(null);});
  // A conflicting edit means another view changed the review: reload it rather than guessing.
  const edit=(change:DuplicateReviewEdit)=>review&&void task.run("Saving choice…",null,async id=>{
    try{setReview(await bridge.editReview(id,review.id,review.revision,change));setPreview(null);}
    catch(e){try{setReview(await bridge.getReview(id,review.id));}catch{/* The original error is more useful. */}throw e;}
  });
  const reviewId=review?.id;
  useEffect(()=>{if(!reviewId||!changeTick)return;let live=true;void task.run("Refreshing review…",null,async id=>{const next=await bridge.getReview(id,reviewId);if(live){setReview(next);setPreview(null);}});return()=>{live=false;};
  // eslint-disable-next-line react-hooks/exhaustive-deps
  },[changeTick]);
  const prefer=()=>{
    const preference:Preference=pref==="most_cuepoints"||pref==="highest_bitrate"?{kind:pref}:pref==="folder"?{kind:"folder",path:prefValue}:{kind:pref,value:prefValue};
    edit({kind:"prefer",preference,override_locked:override,use_best_cues:bestCues});
  };
  const prefNeedsValue=pref!=="most_cuepoints"&&pref!=="highest_bitrate";
  const groups=review?.groups??[],paged=usePaged(groups);
  const titles=useMemo(()=>new Map(groups.flatMap(g=>g.tracks.map(t=>[t.candidate.id,label(t)] as const))),[groups]);
  const playlistNames=useMemo(()=>new Map(playlists.map(p=>[p.id,p.name])),[playlists]);
  const makePreview=()=>review&&void task.run("Planning merge…",null,async id=>{setPreview(await bridge.previewMerge(id,review.id,review.revision,[...chosen]));});
  const apply=()=>preview&&void task.run("Merging duplicates…",null,async id=>{
    const result=await bridge.applyMerge(id,preview.id);
    setNotice(`Merge applied: ${result.archived_ids.length.toLocaleString()} tracks archived, ${result.updated_ids.length.toLocaleString()} updated. No music files were deleted. Undo is in the merge history below.`);
    setPreview(null);setReview(null);setScan(null);setChosen(new Set());onLibraryChanged();
  }).then(()=>setConfirming(false));
  const archiveCount=preview?.plan.groups.reduce((n,g)=>n+g.archive_ids.length,0)??0,copyCount=preview?.plan.groups.reduce((n,g)=>n+g.copies.length,0)??0;
  const idle=!!task.busy;
  return <section className="maint-panel" aria-label="Duplicates">
    <p className="cleanup-note">Find tracks that are the same recording. Review each group, choose what to keep, then preview the exact changes. Merging archives the other copies and can copy tags and cues; it never deletes music files, and Undo restores the original files.</p>
    <div className="maint-controls">
      <label>Scope<select aria-label="Duplicate scope" disabled={idle} value={scope} onChange={e=>setScope(e.target.value as DuplicateScanRequest["scope"])}>{scopes.map(([v,l])=><option key={v} value={v}>{l}</option>)}</select></label>
      <label>Artist/title matching<select aria-label="Match tolerance" disabled={idle} value={tolerance} onChange={e=>setTolerance(e.target.value as DuplicateScanRequest["tolerance"])}>{tolerances.map(([v,l])=><option key={v} value={v}>{l}</option>)}</select></label>
      <button className="primary" disabled={idle||invalid} onClick={runScan}><ScanSearch size={14}/>Scan for duplicates</button>
    </div>
    {needsPlaylists?<fieldset className="maint-playlists" disabled={idle}><legend>Playlists</legend>
      {!playlists.length?<p className="cleanup-note">Create a playlist to scan by playlist.</p>:playlists.map(p=><label key={p.id}><input type="checkbox" checked={playlistIds.includes(p.id)} onChange={e=>setPlaylistIds(old=>e.target.checked?[...old,p.id]:old.filter(v=>v!==p.id))}/>{p.name}</label>)}</fieldset>:null}
    <TaskStatus task={task}/>
    {notice?<div role="status" className="maint-notice">{notice}</div>:null}
    {scan&&!review?<>
      <StaleNotice stale={scan.stale} reason={scan.stale_reason} what="scan"/>
      <div className="maint-summary" role="status"><strong>{scan.groups.length.toLocaleString()} duplicate {scan.groups.length===1?"group":"groups"} among {scan.tracks.length.toLocaleString()} tracks</strong>
        <span>{scan.fingerprints_computed.toLocaleString()} decoded · {scan.cache_hits.toLocaleString()} reused from cache</span></div>
      {scan.tracks.filter(t=>t.state!=="present").length?<details className="maint-issues"><summary>{scan.tracks.filter(t=>t.state!=="present").length} tracks could not be compared</summary><ul>
        {scan.tracks.filter(t=>t.state!=="present").slice(0,200).map(t=><li key={t.id}><code>{t.display_path}</code> {stateLabels[t.state]}{t.detail?`: ${t.detail}`:""}</li>)}</ul></details>:null}
      <button className="primary" disabled={idle||scan.stale||!scan.groups.length} onClick={begin}>Review {scan.groups.length.toLocaleString()} {scan.groups.length===1?"group":"groups"}</button>
    </>:null}
    {review?<>
      <StaleNotice stale={review.stale} reason={review.stale_reason} what="review"/>
      <div className="maint-summary" role="status"><strong>Reviewing {groups.length.toLocaleString()} {groups.length===1?"group":"groups"}</strong><span>{chosen.size.toLocaleString()} selected to merge. Your choices are saved and survive a restart.</span></div>
      <fieldset className="maint-prefer" disabled={idle||review.stale}><legend>Set keepers in bulk</legend>
        <select aria-label="Keeper preference" value={pref} onChange={e=>setPref(e.target.value as Preference["kind"])}>{preferenceKinds.map(([v,l])=><option key={v} value={v}>{l}</option>)}</select>
        {prefNeedsValue?<input aria-label="Preference value" value={prefValue} onChange={e=>setPrefValue(e.target.value)} placeholder={pref==="folder"?"/path/to/folder":"text"}/>:null}
        <label><input type="checkbox" checked={override} onChange={e=>setOverride(e.target.checked)}/>Also change locked groups</label>
        <label><input type="checkbox" checked={bestCues} onChange={e=>setBestCues(e.target.checked)}/>Take cues from the track with the most</label>
        <button className="secondary" disabled={prefNeedsValue&&!prefValue.trim()} onClick={prefer}>Apply to all groups</button></fieldset>
      <div className="maint-controls"><button className="secondary" disabled={idle} onClick={()=>setChosen(old=>old.size?new Set():new Set(groups.filter(mergeable).map(g=>g.id)))}>{chosen.size?"Clear selection":"Select all mergeable"}</button>
        <button className="primary" disabled={idle||review.stale||!chosen.size} onClick={makePreview}><GitMerge size={14}/>Preview merge…</button>
        <button className="secondary" disabled={idle} onClick={()=>{setReview(null);setPreview(null);}}>Close review</button></div>
      {paged.visible.map(g=><GroupCard key={g.id} group={g} disabled={idle||review.stale} selected={chosen.has(g.id)} onSelect={on=>{setPreview(null);setChosen(old=>{const next=new Set(old);if(on)next.add(g.id);else next.delete(g.id);return next;});}} onEdit={change=>edit({kind:"group",group_id:g.id,edit:change})}/>)}
      <ShowMore {...paged}/>
    </>:null}
    {preview?<section className="maint-preview" aria-label="Merge preview"><h2>Review merge</h2>
      <p>{preview.plan.groups.length} {preview.plan.groups.length===1?"group":"groups"}: {archiveCount} tracks will be archived and {copyCount} tag/cue copies made. No music files are deleted.</p>
      {preview.plan.groups.map(g=><article key={g.group_id} className="maint-plan"><strong>Keep {titles.get(g.keeper)??g.keeper}</strong><small>{g.mode==="copy_cues"?"Copy tags and cues only":g.exact_audio?"Identical audio":"Artist/title match"}{g.unarchive_keeper?" · restores the archived keeper":""}</small>
        {g.archive_ids.length?<ul className="maint-list" aria-label="Tracks to archive">{g.archive_ids.map(id=><li key={id}><span>{titles.get(id)??id}</span><small>Archive</small></li>)}</ul>:null}
        {g.copies.length?<ul className="maint-list" aria-label="Tags and cues to copy">{g.copies.map(c=><li key={c.from+c.to}><span>{titles.get(c.from)??c.from} → {titles.get(c.to)??c.to}</span><small>Copy tags and cues</small></li>)}</ul>:null}</article>)}
      {preview.playlists.length?<><h3>Playlist changes</h3><ul className="maint-list" aria-label="Playlist changes">{preview.playlists.map(p=><li key={p.id}><span>{playlistNames.get(p.id)??p.id}</span><small>{p.before.length} → {p.after.length} tracks</small></li>)}</ul></>:null}
      <div className="maint-controls"><button className="secondary" onClick={()=>setPreview(null)}>Cancel</button><button className="primary" disabled={idle||review?.stale} onClick={()=>setConfirming(true)}>Apply merge…</button></div></section>:null}
    {confirming&&preview?<ConfirmDialog title="Apply this merge?" confirmLabel="Apply merge" busy={idle} onConfirm={apply} onCancel={()=>setConfirming(false)}>
      <p>{archiveCount} tracks will be archived and {copyCount} tag/cue copies written to the kept files. The original file bytes are backed up first, and Undo in the merge history restores them.</p><TaskStatus task={task}/></ConfirmDialog>:null}
    <DuplicateExtras bridge={bridge} task={task} changeTick={changeTick} onLibraryChanged={onLibraryChanged} onNotice={setNotice}/>
  </section>;
}
