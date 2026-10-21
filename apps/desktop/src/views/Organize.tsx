import {useEffect,useRef,useState,useCallback} from "react";
import {ScanLine,Undo2,RefreshCw,X,FolderOpen,ChevronLeft} from "lucide-react";
import type {Bridge,Snapshot} from "../types";
import type {PlaylistWorkspace} from "../playlist-types";
import {defaultOrganization} from "../organization-types";
import type {OrganizationSettings as Config,OrganizationScope,OrganizationPreview,OrganizationPage,OrganizationBatch,OrganizationRoot,OrganizationAlternate} from "../organization-types";
import OrganizationSettings from "./OrganizationSettings";
import {errorMessage} from "../api";
const pageSize=50;
const storageSize=(bytes:number)=>{const power=Math.min(4,Math.max(0,Math.floor(Math.log2(Math.max(1,bytes))/10)));return `${new Intl.NumberFormat(undefined,{maximumFractionDigits:1}).format(bytes/1024**power)} ${["B","KiB","MiB","GiB","TiB"][power]}`;};
export default function Organize({bridge,snapshot,selectedIds,playlistId,workspace,onChanged,onBack,revision}:{bridge:Bridge;snapshot:Snapshot;selectedIds:string[];playlistId:string|null;workspace:PlaylistWorkspace|null;onChanged:()=>void;onBack?:()=>void;revision?:number}){
 const [config,setConfig]=useState<Config>(()=>snapshot.settings.organization??defaultOrganization());
 const [scope,setScope]=useState(selectedIds.length?"selected":playlistId?"playlist":"all"),[root,setRoot]=useState<OrganizationRoot|null>(null),[recursive,setRecursive]=useState(true);
 const [preview,setPreview]=useState<OrganizationPreview|null>(null),[page,setPage]=useState<OrganizationPage|null>(null),[detail,setDetail]=useState<OrganizationBatch|null>(null),[history,setHistory]=useState<OrganizationBatch[]>([]),[alternates,setAlternates]=useState<OrganizationAlternate[]>([]);
 const [busy,setBusy]=useState<string|null>(null),[operationId,setOperationId]=useState<string|null>(null),[error,setError]=useState<string|null>(null),[validation,setValidation]=useState<string|null>(null),[pageBusy,setPageBusy]=useState(false);
 const previousRevision=useRef(revision);
 const detailPage=useRef<{id:string;offset:number}|null>(null);
 const pageFlight=useRef<{id:string;offset:number;isBatch:boolean;request:number;refresh:boolean}|null>(null);
 const alive=useRef(true),generation=useRef(0),pageRequest=useRef(0),historyRequest=useRef(0),previewRef=useRef<OrganizationPreview|null>(null),planning=useRef<string|null>(null),slot=useRef(false);
 const validationChanged=useCallback((value:string|null)=>setValidation(value),[]);
 const refreshHistory=useCallback(async()=>{const current=++historyRequest.current;const [batches,conflicts]=await Promise.all([bridge.organizationHistory(),bridge.organizationAlternates()]);if(alive.current&&current===historyRequest.current){setHistory(batches);setAlternates(conflicts.filter(conflict=>conflict.state==="conflict"));setDetail(previous=>previous?batches.find(b=>b.id===previous.id)??previous:previous);}},[bridge]);
 useEffect(()=>{alive.current=true;void refreshHistory().catch(e=>{if(alive.current)setError(errorMessage(e));});return()=>{alive.current=false;generation.current++;pageRequest.current++;if(planning.current)void bridge.cancelOrganization(planning.current).catch(()=>{});if(previewRef.current)void bridge.discardOrganizationPreview(previewRef.current.id).catch(()=>{});};},[bridge,refreshHistory]);
 useEffect(()=>{if(!history.some(batch=>batch.state==="applying"))return;const timer=setInterval(()=>void refreshHistory().catch(e=>{if(alive.current)setError(errorMessage(e));}),1200);return()=>clearInterval(timer);},[history,refreshHistory]);
 const discard=()=>{generation.current++;pageRequest.current++;detailPage.current=null;setPageBusy(false);const previous=previewRef.current;previewRef.current=null;setPreview(null);setPage(null);setDetail(null);if(previous)void bridge.discardOrganizationPreview(previous.id).catch(e=>{if(alive.current)setError(errorMessage(e));});};
 useEffect(()=>{if(previousRevision.current!==revision){previousRevision.current=revision;if(previewRef.current||planning.current){discard();setError("Library changed. Preview the complete plan again before applying.");}void refreshHistory().catch(e=>{if(alive.current)setError(errorMessage(e));});}},[revision,refreshHistory]);
 const update=(next:Config)=>{discard();setConfig(next);};
 const readPage=async(id:string,offset:number,isBatch=false):Promise<void>=>{
  if(isBatch)detailPage.current={id,offset};
  const pending=pageFlight.current;
  if(pending&&pending.id===id&&pending.offset===offset&&pending.isBatch===isBatch&&pending.request===pageRequest.current){pending.refresh=true;return;}
  const request=++pageRequest.current,flight={id,offset,isBatch,request,refresh:false};pageFlight.current=flight;setPageBusy(true);
  try{const next=await (isBatch?bridge.organizationBatchPage(id,offset,pageSize):bridge.organizationPreviewPage(id,offset,pageSize));if(alive.current&&request===pageRequest.current)setPage(next);}
  catch(e){if(alive.current&&request===pageRequest.current)setError(errorMessage(e));}
  finally{
   if(alive.current&&request===pageRequest.current)setPageBusy(false);
   if(pageFlight.current===flight){pageFlight.current=null;if(flight.refresh&&alive.current&&request===pageRequest.current)void readPage(id,offset,isBatch);}
  }
 };
 useEffect(()=>{if(detail)void readPage(detail.id,detailPage.current?.id===detail.id?detailPage.current.offset:0,true);},[detail]);
 const run=async(label:string,operation:(id:string)=>Promise<void>)=>{if(slot.current)return;slot.current=true;const id=crypto.randomUUID();setOperationId(id);setBusy(label);setError(null);try{await operation(id);}catch(e){if(alive.current)setError(errorMessage(e));}finally{slot.current=false;if(alive.current){setBusy(null);setOperationId(null);}}};
 const scan=()=>run("Checking every track…",async id=>{
  discard();const current=++generation.current;planning.current=id;
  let selection:OrganizationScope={kind:"all_ready"};
  if(scope==="selected")selection={kind:"tracks",track_ids:selectedIds};
  if(scope==="playlist"&&playlistId)selection={kind:"playlist",playlist_id:playlistId};
  if(scope==="folder"&&root)selection={kind:"folder",root_id:root.id,recursive};
  try{const next=await bridge.previewOrganization(id,{scope:selection,settings:config});if(!alive.current||current!==generation.current){await bridge.discardOrganizationPreview(next.id);return;}previewRef.current=next;setPreview(next);await readPage(next.id,0);}finally{if(planning.current===id)planning.current=null;}
 });
 const refreshAfter=async(action:()=>Promise<void>)=>{
  let failure:{error:unknown}|null=null;
  try{await action();}catch(error){failure={error};}
  if(alive.current){
   onChanged();
   try{await refreshHistory();}catch(error){if(!failure)throw error;}
  }
  if(failure)throw failure.error;
 };
 const mutate=async(id:string,action:()=>Promise<OrganizationBatch>)=>{
  // A rejected invocation may already have published files. Its old plan cannot
  // authorize another Apply until the native service establishes fresh evidence.
  pageRequest.current++;setPageBusy(false);previewRef.current=null;setPreview(null);setPage(null);
  await refreshAfter(async()=>{
   const next=await action();
   if(alive.current){setDetail(next);setHistory(old=>[next,...old.filter(b=>b.id!==next.id)]);setOperationId(next.state==="applying"?id:null);}
  });
 };
 const active=history.find(batch=>batch.state==="applying");
 const disabled=!!busy||!!active;
 const chosenPlaylist=workspace?.tree.nodes.find(node=>node.id===playlistId&&!node.deleted);
 const totals=preview?.totals;
 return <section className="organization-view">
  <div className="view-title"><div>{onBack?<button className="back-link" onClick={onBack}><ChevronLeft size={15}/>Library</button>:<div className="eyebrow">LIBRARY TOOLS</div>}<h1>Organize files</h1><p>Review complete naming plans before changing your library.</p></div><button className="secondary" disabled={!!busy} onClick={()=>void refreshHistory().catch(e=>setError(errorMessage(e)))}><RefreshCw size={14}/>Refresh history</button></div>
  <div className="organization-body">
   {error?<div role="alert" className="error-banner">{error}</div>:null}
   {busy?<p role="status">{busy}</p>:null}
   {active?<p role="status">An organization operation is applying. It continues if you leave this page. Use history to check its result.</p>:null}
   {(operationId||active)?<button className="secondary" onClick={()=>{if(planning.current)generation.current++;void (operationId?bridge.cancelOrganization(operationId):bridge.cancelOrganizationBatch(active!.id)).then(()=>{if(alive.current)setError("Cancellation requested. Completed changes remain in history.");}).catch(e=>setError(errorMessage(e)));}}><X size={14}/>Cancel operation</button>:null}
   <fieldset className="organization-controls" disabled={disabled}>
    <div className="organization-scope"><label>Scope<select aria-label="Organization scope" value={scope} onChange={e=>{discard();setScope(e.target.value);}}><option value="selected" disabled={!selectedIds.length}>{selectedIds.length} selected tracks</option><option value="playlist" disabled={!chosenPlaylist}>{chosenPlaylist?"Current playlist: "+chosenPlaylist.name:"Current playlist"}</option><option value="folder">Folder</option><option value="all">All ready tracks</option></select></label>{scope==="folder"?<><button className="secondary" onClick={async()=>{try{const selected=await bridge.chooseOrganizationRoot();if(selected){discard();setRoot(selected);}}catch(e){setError(errorMessage(e));}}}><FolderOpen size={14}/>Choose scope folder</button><span className="organization-path">{root?.display_path??"No folder chosen"}</span><label><input type="checkbox" checked={recursive} onChange={e=>{discard();setRecursive(e.target.checked);}}/>Include subfolders</label></>:null}</div>
    <details className="organization-rules" open={config.mode==="leave_in_place"}><summary>Naming rules · {config.mode.replaceAll("_"," ")}</summary><OrganizationSettings bridge={bridge} config={config} onChange={update} onValidation={validationChanged}/></details>
    <p className="setting-note">Rules here apply to this manual plan. Save automatic Incoming rules in Settings. A playlist or folder scope includes every matching track, across pages.</p>
    <button className="primary" disabled={!!validation||config.mode==="leave_in_place"||(scope==="folder"&&!root)||(scope==="selected"&&!selectedIds.length)||(scope==="playlist"&&!chosenPlaylist)} onClick={()=>void scan()}><ScanLine size={15}/>Preview complete plan</button>
   </fieldset>
   {preview&&totals?<section className="organization-preview" aria-label="Complete organization preview"><div className="organization-preview-heading"><h2>Complete plan · {totals.total} tracks</h2><button className="primary" disabled={disabled||pageBusy||!preview.can_apply||!totals.ready} onClick={()=>void run("Applying organization…",id=>mutate(id,()=>bridge.applyOrganization(id,preview.id,preview.revision)))}>Apply {totals.ready} eligible</button><button className="secondary" disabled={disabled} onClick={discard}>Discard preview</button></div><p className="organization-totals">{totals.ready} eligible · {totals.unchanged} unchanged · {totals.skipped} skipped · <span className={totals.blocked?"attention":""}>{totals.blocked} blocked</span></p><p className="setting-note">Blocked and skipped tracks stay in place. Existing files are never overwritten. Known linked references must be repaired; arbitrary external libraries cannot be checked.</p></section>:null}
   {preview?.estimated_bytes!=null?<p className="setting-note" aria-label="Organization storage estimate">Estimated additional space: {storageSize(preview.estimated_bytes)}{preview.available_bytes!=null?` · ${storageSize(preview.available_bytes)} minimum available`:""}. Includes staging and retained recovery backups.</p>:null}
   {detail?<section aria-label="Organization result"><h2>{detail.kind==="undo"?"Undo":"Organization"} · {detail.state.replaceAll("_"," ")}</h2><p>{detail.totals.completed} completed · {detail.totals.unchanged} unchanged · {detail.totals.skipped} skipped · {detail.totals.blocked} blocked · {detail.totals.failed} failed · {detail.totals.recovery_required} need recovery · {detail.totals.cancelled} cancelled / {detail.totals.total} total</p>{detail.error?<p role="alert">{detail.error}</p>:null}</section>:null}
   {page&&(preview||detail)?<section className="organization-rows" aria-label="Organization paths" aria-busy={pageBusy}>{pageBusy?<p role="status">Loading paths…</p>:null}{page.rows.map(row=><article key={row.id}><div><strong>{row.title}</strong><span className="organization-path muted">{row.before_path}</span><span className="organization-path">→ {row.after_path??"Unchanged"}</span>{row.reason?<p className={row.outcome==="blocked"?"attention":"muted"}>{row.reason}</p>:null}{row.references.length?<ul className="organization-reference-list">{row.references.map(reference=><li key={reference.id}>{reference.label} · {reference.state}{reference.reason?" — "+reference.reason:""}</li>)}</ul>:null}</div><span className={row.outcome==="blocked"?"attention":"muted"}>{row.outcome.replaceAll("_"," ")}</span></article>)}<div className="organization-pagination"><span>{page.total?`${page.offset+1}–${Math.min(page.total,page.offset+page.rows.length)}`:"0"} of {page.total} tracks</span><button className="secondary" disabled={disabled||pageBusy||page.offset===0} onClick={()=>void readPage((preview??detail)!.id,Math.max(0,page.offset-pageSize),!!detail)}>Previous</button><button className="secondary" disabled={disabled||pageBusy||page.offset+page.limit>=page.total} onClick={()=>void readPage((preview??detail)!.id,page.offset+page.limit,!!detail)}>Next</button></div></section>:null}
   <section className="organization-history" aria-label="Organization history">
    <h2>History and recovery</h2>
    {history.some(batch=>batch.can_recover)?<p className="setting-note">Cancel pending files keeps completed changes. Already published files that need recovery remain in history.</p>:null}
    {!history.length?<p className="setting-note">Applied operations and recovery results appear here.</p>:history.map(batch=><article key={batch.id}>
     <div><strong>{batch.kind==="undo"?"Undo":"Organization"} · {batch.state.replaceAll("_"," ")}</strong><small>{new Date(batch.created).toLocaleString()} · {batch.totals.completed} of {batch.totals.total} completed</small>{batch.error?<p className="attention">{batch.error}</p>:null}</div>
     <div className="organization-history-actions">
      <button className="secondary" disabled={!!busy} onClick={()=>{discard();setDetail(batch);}}>Details</button>
      {batch.can_recover?<>
       {batch.resume_job_id?<button className="secondary" disabled={disabled} onClick={()=>void run("Resuming Incoming preparation…",()=>refreshAfter(()=>bridge.retryJob(batch.resume_job_id!,true)))}>Retry in Incoming</button>:<button className="secondary" disabled={disabled} onClick={()=>void run("Recovering organization…",id=>mutate(id,()=>bridge.recoverOrganization(id,batch.id)))}>Retry recovery</button>}
       <button className="secondary" disabled={disabled} onClick={()=>void run("Cancelling pending files…",()=>refreshAfter(()=>bridge.cancelOrganizationBatch(batch.id)))}>Cancel pending files</button>
      </>:null}
      {batch.can_undo?<button className="secondary" disabled={disabled} onClick={()=>void run("Checking and undoing…",id=>mutate(id,()=>bridge.undoOrganization(id,batch.id)))}><Undo2 size={14}/>Undo</button>:null}
     </div>
    </article>)}
   </section>
   {alternates.length?<section aria-label="Changed original files"><h2>Changed original files</h2><p className="setting-note">An original has changed since its managed copy was created. Choose which version belongs in the library.</p>{alternates.map(alternate=><article className="organization-alternate" key={alternate.operation_id}><p className="organization-path">{alternate.display_path}</p><div className="organization-history-actions">{([['ignore_this_revision','Keep managed copy'],['add_as_new_incoming','Add changed original as Incoming']] as const).map(([resolution,label])=><button className="secondary" key={resolution} disabled={disabled} onClick={()=>void run("Resolving changed original…",operationId=>refreshAfter(()=>bridge.resolveOrganizationAlternate(alternate.operation_id,resolution,operationId)))}>{label}</button>)}</div></article>)}</section>:null}
  </div>
 </section>;
}
