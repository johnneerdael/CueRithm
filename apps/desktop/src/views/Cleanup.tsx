import {useEffect,useRef,useState} from "react";
import {ScanLine,Save,Undo2,LoaderCircle} from "lucide-react";
import type {Bridge,Snapshot,CleanupConfig,CleanupPreview,CleanupBatchResult,CleanupHistory,CleanupCatalog} from "../types";
import {errorMessage} from "../api";
import {emptyCleanup,SmartFixEditor,CanonicalEditor} from "./CleanupSettings";

function HistoryDetails({history,bridge}:{history:CleanupHistory;bridge:Bridge}){
 const [expanded,setExpanded]=useState(false),[titles,setTitles]=useState<Record<string,string>>({});
 const problems=[...history.result.conflicted.map(f=>({...f,kind:"Conflict"})),...history.result.failed.map(f=>({...f,kind:"Failed"}))];
 const ids=[...new Set(problems.map(f=>f.track_id))],signature=JSON.stringify(ids);
 useEffect(()=>{
  if(!expanded)return;let canceled=false,next=0;
  // Fetch display names only for an opened report, with bounded concurrency.
  const read=async()=>{while(!canceled&&next<ids.length){const id=ids[next++];try{const detail=await bridge.getTrack(id);if(!canceled)setTitles(old=>({...old,[id]:detail.track.title}));}catch{/* Stable IDs remain available if a track cannot be loaded. */}}};
  void Promise.all(Array.from({length:Math.min(4,ids.length)},read));
  return()=>{canceled=true;};
 },[bridge,expanded,signature]);
 if(!problems.length)return null;
 return <div className="cleanup-history-details"><button className="back-link" aria-expanded={expanded} onClick={()=>setExpanded(v=>!v)}>{history.result.conflicted.length} {history.result.conflicted.length===1?"conflict":"conflicts"} · {history.result.failed.length} failed</button>{expanded?<ul>{problems.map((f,i)=><li key={f.track_id+":"+i}><strong>{titles[f.track_id]??f.track_id}</strong>{titles[f.track_id]?<small>{f.track_id}</small>:null}<span>{f.kind}: <span>{f.message}</span></span></li>)}</ul>:null}</div>;
}

export default function Cleanup({bridge,snapshot,selectedIds,onChanged}:{bridge:Bridge;snapshot:Snapshot;selectedIds:string[];onChanged:()=>void}){
 const [config,setConfig]=useState<CleanupConfig>(()=>snapshot.settings.cleanup??emptyCleanup());
 const [baseline,setBaseline]=useState(()=>JSON.stringify(snapshot.settings.cleanup??emptyCleanup()));
 const [settings,setSettings]=useState(snapshot.settings);
 const [tab,setTab]=useState<"fixes"|"genres"|"artists">("fixes");
 const [scope,setScope]=useState(selectedIds.length?"selected":"all");
 const [preview,setPreview]=useState<CleanupPreview|null>(null),[checked,setChecked]=useState(new Set<string>());
 const [result,setResult]=useState<CleanupBatchResult|null>(null),[history,setHistory]=useState<CleanupHistory[]>([]);
 const [catalog,setCatalog]=useState<CleanupCatalog|null>(null),[error,setError]=useState<string|null>(null);
 const [busy,setBusy]=useState<string|null>(null),[catalogBusy,setCatalogBusy]=useState(false);
 const generation=useRef(0),catalogRequest=useRef(0),alive=useRef(true);
 const dirty=JSON.stringify(config)!==baseline;
 const configRef=useRef(config);configRef.current=config;
 const update=(next:CleanupConfig)=>{generation.current++;setConfig(next);setPreview(null);setResult(null);};
 useEffect(()=>{
  alive.current=true;let canceled=false;
  void bridge.cleanupHistory().then(h=>{if(!canceled)setHistory(h);}).catch(e=>{if(!canceled)setError(errorMessage(e));});
  return()=>{canceled=true;alive.current=false;generation.current++;catalogRequest.current++;};
 },[bridge]);
 useEffect(()=>{
  const next=snapshot.settings.cleanup??emptyCleanup();
  generation.current++;setPreview(null);setConfig(next);setBaseline(JSON.stringify(next));setSettings(snapshot.settings);
 },[snapshot.revision]);
 const loadCatalog=async()=>{
  const request=++catalogRequest.current;setCatalogBusy(true);
  try{const next=await bridge.cleanupCatalog(configRef.current);if(alive.current&&request===catalogRequest.current)setCatalog(next);}
  catch(e){if(alive.current&&request===catalogRequest.current)setError(errorMessage(e));}
  finally{if(alive.current&&request===catalogRequest.current)setCatalogBusy(false);}
 };
 useEffect(()=>{if(tab!=="fixes")void loadCatalog();},[tab]);
 const run=async(label:string,operation:()=>Promise<void>)=>{
  if(busy)return;setBusy(label);setError(null);
  try{await operation();}catch(e){if(alive.current)setError(errorMessage(e));}
  finally{if(alive.current)setBusy(null);}
 };
 const save=()=>run("Saving rules…",async()=>{
  const saved=await bridge.saveSettings({...settings,cleanup:config});
  if(!alive.current)return;setSettings(saved.settings);setBaseline(JSON.stringify(saved.settings.cleanup??emptyCleanup()));onChanged();
 });
 const scan=()=>run("Scanning tracks…",async()=>{
  const current=++generation.current;setPreview(null);setResult(null);
  const next=await bridge.previewCleanup(scope==="selected"?{kind:"track_ids",track_ids:selectedIds}:{kind:"all_ready"},config);
  if(!alive.current||current!==generation.current)return;setPreview(next);setChecked(new Set(next.proposals.map(p=>p.id)));
 });
 const afterWrite=async(next:CleanupBatchResult)=>{
  if(!alive.current)return;setResult(next);setPreview(null);onChanged();
  const h=await bridge.cleanupHistory();if(alive.current)setHistory(h);
  await loadCatalog();
 };
 const failures=result?[...result.conflicted,...result.failed]:preview?.failed??[];
 return <section className="cleanup-view">
  <div className="view-title"><div><div className="eyebrow">LIBRARY TOOLS</div><h1>Cleanup</h1><p>Review metadata changes. Keep your audio, analysis and cues.</p></div><button className="secondary" disabled={!!busy||!dirty} onClick={()=>void save()}><Save size={14}/>Save cleanup rules</button></div>
  <div className="cleanup-tabs" role="tablist" aria-label="Cleanup tools">{([['fixes','Smart Fixes'],['genres','Genres'],['artists','Artists']] as const).map(([id,label])=><button key={id} role="tab" aria-selected={tab===id} onClick={()=>setTab(id)}>{label}</button>)}</div>
  <div className="cleanup-body">
   {error?<div role="alert" className="error-banner">{error}</div>:null}
   {busy?<div role="status" className="cleanup-status"><LoaderCircle className="spin" size={15}/>{busy}</div>:null}
   <fieldset className="cleanup-controls" disabled={!!busy}>
    {tab==="fixes"?<SmartFixEditor config={config} onChange={update}/>:<><CanonicalEditor kind={tab} config={config} onChange={update} catalog={catalog}/>{catalogBusy?<p className="cleanup-note">Reading library values…</p>:<button className="back-link" onClick={()=>void loadCatalog()}>Refresh values</button>}{catalog?.unavailable_tracks.length?<p className="attention">{catalog.unavailable_tracks.length} tracks unavailable for metadata reading.</p>:null}</>}
   </fieldset>
   <div className="cleanup-scan"><label>Scope<select aria-label="Cleanup scope" disabled={!!busy} value={scope} onChange={e=>{generation.current++;setScope(e.target.value);setPreview(null);}}><option value="all">All ready tracks</option><option value="selected" disabled={!selectedIds.length}>{selectedIds.length} selected tracks</option></select></label><button className="primary" disabled={!!busy||dirty||(scope==="selected"&&!selectedIds.length)} onClick={()=>void scan()}><ScanLine size={15}/>Scan tracks</button>{dirty?<span className="attention">Save rule changes before scanning.</span>:<small>Scanning previews changes without writing file tags.</small>}</div>
   {preview?<section className="cleanup-preview" aria-label="Cleanup preview"><div className="cleanup-preview-heading"><h2>{preview.proposals.length} proposed {preview.proposals.length===1?"change":"changes"}</h2><button className="primary" disabled={!!busy||!checked.size} onClick={()=>void run("Applying cleanup…",async()=>afterWrite(await bridge.applyCleanup(preview.id,[...checked])))}>Apply {checked.size} {checked.size===1?"change":"changes"}</button></div>
    <p className="cleanup-note">{preview.unchanged.length} unchanged · {preview.failed.length} unavailable. Apply updates file tags and library metadata with Undo.</p>
    {preview.proposals.length?<label className="cleanup-select-all"><input type="checkbox" checked={checked.size===preview.proposals.length} onChange={e=>setChecked(new Set(e.target.checked?preview.proposals.map(p=>p.id):[]))}/>Select all proposals</label>:<p>No metadata changes needed with these rules.</p>}
    {preview.proposals.map(p=><article className="cleanup-proposal" key={p.id}><label><input type="checkbox" disabled={!!busy} aria-label={"Apply cleanup to "+p.title} checked={checked.has(p.id)} onChange={e=>setChecked(old=>{const next=new Set(old);if(e.target.checked)next.add(p.id);else next.delete(p.id);return next;})}/><strong>{p.title}</strong></label><dl>{p.report.changes.map(c=><div key={c.field}><dt>{c.field.replaceAll("_"," ")}</dt><dd className="before">{c.before.join(" · ")||"Empty"}</dd><dd className="after">{c.after.join(" · ")||"Empty"}</dd></div>)}{JSON.stringify(p.report.original.custom_tags)!==JSON.stringify(p.report.metadata.custom_tags)?<div><dt>Custom tags</dt><dd className="before">{p.report.original.custom_tags.map(t=>t.category+": "+t.value).join(" · ")||"Empty"}</dd><dd className="after">{p.report.metadata.custom_tags.map(t=>t.category+": "+t.value).join(" · ")||"Empty"}</dd></div>:null}</dl></article>)}
   </section>:null}
   {result?<div className="cleanup-result"><h2>Cleanup result</h2><p>{result.applied.length} applied · {result.unchanged.length} unchanged · {result.conflicted.length} conflicts · {result.failed.length} failed</p>{result.conflicted.length?<p>Changed tracks were preserved. Scan again for a fresh preview.</p>:null}</div>:null}
   {failures.length?<ul className="cleanup-failures">{failures.map((f,i)=><li key={f.track_id+":"+i}>{f.message}</li>)}</ul>:null}
   <section className="cleanup-history"><h2>Undo history</h2>{!history.length?<p className="cleanup-note">Applied changes and recovery results will appear here.</p>:history.map(h=><div className="cleanup-history-entry" key={h.id}><div className="cleanup-history-row"><div><strong>{h.kind==="undo"?"Undo":"Cleanup"}</strong><small>{new Date(h.created).toLocaleString()} · {h.state} · {h.result.applied.length} {h.result.applied.length===1?"track":"tracks"}</small></div>{h.kind==="apply"&&h.can_undo?<button className="secondary" disabled={!!busy} aria-label={"Undo "+h.id} onClick={()=>void run("Undoing cleanup…",async()=>afterWrite(await bridge.undoCleanup(h.id)))}><Undo2 size={14}/>Undo</button>:null}</div><HistoryDetails history={h} bridge={bridge}/></div>)}</section>
  </div>
 </section>;
}
