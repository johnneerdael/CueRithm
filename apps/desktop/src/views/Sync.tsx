import {useCallback,useEffect,useRef,useState} from "react";
import {ArrowRightLeft,FolderOpen,LoaderCircle,RefreshCw,RotateCcw,Search,Trash2} from "lucide-react";
import type {Bridge} from "../types";
import type {PlaylistWorkspace} from "../playlist-types";
import {errorMessage} from "../api";
import {defaultSyncOptions,syncTargets} from "../sync-types";
import type {DjSoftware,DjSyncHistoryEntry,DiscoveredTarget,DjSyncRequest,SyncOptions,SyncPreview,SyncTextField} from "../sync-types";

const fields:SyncTextField[]=["title","artist","album","genre","label","comment","key"];
function size(bytes:number){return bytes<1024?`${bytes} B`:bytes<1048576?`${(bytes/1024).toFixed(1)} KB`:`${(bytes/1048576).toFixed(1)} MB`;}
export default function Sync({bridge,workspace,revision,onChanged}:{bridge:Bridge;workspace:PlaylistWorkspace|null;revision:number;onChanged:()=>void}){
 const [software,setSoftware]=useState<DjSoftware>("virtual_dj"),[path,setPath]=useState("");
 const [targets,setTargets]=useState<DiscoveredTarget[]>([]),[scope,setScope]=useState<"full"|"playlist"|"changed">("full"),[playlistId,setPlaylistId]=useState("");
 const [options,setOptions]=useState<SyncOptions>({...defaultSyncOptions}),[preview,setPreview]=useState<SyncPreview|null>(null);
 const [history,setHistory]=useState<DjSyncHistoryEntry[]>([]),[busy,setBusy]=useState<string|null>(null),[error,setError]=useState<string|null>(null),[result,setResult]=useState<string|null>(null);
 const alive=useRef(true),generation=useRef(0),activePreview=useRef<string|null>(null),operation=useRef(false),historyRequest=useRef(0);
 const info=syncTargets.find(t=>t.id===software)!;
 const playlists=workspace?.tree.nodes.filter(n=>!n.deleted)??[];
 const signature=JSON.stringify({software,path,scope,playlistId,options,revision});
 const reloadHistory=useCallback(async()=>{const current=++historyRequest.current;const next=await bridge.djSyncHistory();if(alive.current&&current===historyRequest.current)setHistory(next);},[bridge]);
 const discardPrepared=useCallback(async(id:string)=>{await bridge.discardDjSync(id);if(alive.current)await reloadHistory();},[bridge,reloadHistory]);
 const clearPreview=useCallback(()=>{const id=activePreview.current;activePreview.current=null;setPreview(null);return id;},[]);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;historyRequest.current++;const id=activePreview.current;activePreview.current=null;if(id)void bridge.discardDjSync(id).catch(()=>{});};},[bridge]);
 useEffect(()=>{generation.current++;const id=clearPreview();if(id)void discardPrepared(id).catch(e=>{if(alive.current)setError(errorMessage(e));});},[signature,clearPreview,discardPrepared]);
 const discover=useCallback(async()=>{try{const next=await bridge.discoverDjTargets();if(alive.current)setTargets(next);}catch(e){if(alive.current)setError(errorMessage(e));}},[bridge]);
 useEffect(()=>{void discover();void reloadHistory().catch(e=>{if(alive.current)setError(errorMessage(e));});},[discover,reloadHistory]);
 useEffect(()=>{if(!path){const found=targets.find(t=>t.software===software);if(found)setPath(found.path);}},[software,targets,path]);
 const update=<K extends keyof SyncOptions>(key:K,value:SyncOptions[K])=>setOptions(previous=>({...previous,[key]:value}));
 const selected=scope==="playlist"?playlists.find(n=>n.id===playlistId):null;
 const canPreview=!!path.trim()&&(scope!=="playlist"||!!selected);
 const run=async(action:string,work:()=>Promise<void>)=>{if(operation.current)return;operation.current=true;setBusy(action);setError(null);setResult(null);try{await work();}catch(e){if(alive.current)setError(errorMessage(e));}finally{operation.current=false;if(alive.current){setBusy(null);void reloadHistory().catch(e=>{if(alive.current)setError(errorMessage(e));});}}};
 const prepare=()=>run("preview",async()=>{
  const request:DjSyncRequest={target:{software,path:path.trim(),source_root:null},scope:scope==="playlist"?{type:"playlist",id:playlistId}:{type:scope},options};
  const current=++generation.current;const previous=clearPreview();if(previous)await discardPrepared(previous);
  if(!alive.current||current!==generation.current)return;
  const next=await bridge.previewDjSync(request);
  if(alive.current&&current===generation.current){activePreview.current=next.id;setPreview(next);}else await discardPrepared(next.id);
 });
 const releasePreview=async(preserve:string)=>{generation.current++;const id=clearPreview();if(id&&id!==preserve)await discardPrepared(id);};
 const apply=()=>{if(!preview||activePreview.current!==preview.id)return;const id=preview.id;void run("apply",async()=>{await releasePreview(id);const completed=await bridge.applyDjSync(id);if(alive.current){setResult(completed.changes.length||completed.renames?.length||completed.directories.some(d=>!d.before_manifest||!d.after_exists)?"Sync complete. Your backup is available below.":"Everything is already synced.");onChanged();}});};
 const complete=(id:string)=>run("acknowledge",async()=>{await releasePreview(id);await bridge.applyDjSync(id);if(alive.current){setResult("Sync recovered. Your backup is available below.");onChanged();}});
 const restore=(id:string)=>run("restore",async()=>{await releasePreview(id);await bridge.restoreDjSync(id);if(alive.current){setResult("Backup restored.");onChanged();}});
 const discard=(id:string)=>run("discard",async()=>{if(activePreview.current===id){generation.current++;clearPreview();}await discardPrepared(id);if(alive.current)setResult("Preview discarded.");});
 return <section className="sync-view">
  <header className="view-title"><div><div className="eyebrow">DJ LIBRARIES</div><h1>Sync</h1><p>Send prepared tracks, cues and playlists to your DJ library.</p></div><ArrowRightLeft className="sync-heading-icon" size={27}/></header>
  <div className="sync-body">
   {error?<div className="error-banner" role="alert">{error}</div>:null}
   {result?<p className="sync-result" role="status">{result}</p>:null}
   <fieldset className="sync-controls" disabled={!!busy}>
    <div className="sync-destination">
     <label>Sync to<select aria-label="Sync to" value={software} onChange={e=>{setSoftware(e.target.value as DjSoftware);setPath("");setOptions({...defaultSyncOptions});setResult(null);}}>{syncTargets.map(t=><option key={t.id} value={t.id}>{t.name}</option>)}</select></label>
     <label>{info.location}<div className="sync-path"><input aria-label={info.location} placeholder={software==="pioneer_usb"?"Choose a mounted USB drive":"Choose your DJ library"} value={path} onChange={e=>setPath(e.target.value)}/><button type="button" className="secondary" aria-label="Choose sync destination" onClick={()=>void bridge.chooseFolder().then(value=>{if(value&&alive.current)setPath(value);}).catch(e=>setError(errorMessage(e)))}><FolderOpen size={15}/></button><button type="button" className="secondary" aria-label="Refresh DJ libraries" onClick={()=>void discover()}><RefreshCw size={14}/></button></div></label>
     {targets.filter(t=>t.software===software).length>1?<label>Found libraries<select aria-label="Found libraries" value={path} onChange={e=>setPath(e.target.value)}><option value="">Choose a library</option>{targets.filter(t=>t.software===software).map(t=><option key={t.path} value={t.path}>{t.path}</option>)}</select></label>:null}
     <label>What to sync<select aria-label="What to sync" value={scope} onChange={e=>setScope(e.target.value as typeof scope)}><option value="full">Full library</option><option value="playlist">Playlist or folder</option><option value="changed">Modified since last sync</option></select></label>
     {scope==="playlist"?<label>Playlist<select aria-label="Sync playlist" value={playlistId} onChange={e=>setPlaylistId(e.target.value)}><option value="">Choose a playlist or folder</option>{playlists.map(n=><option key={n.id} value={n.id}>{n.name}</option>)}</select></label>:null}
     <p className="sync-note">Close {software==="pioneer_usb"?"Rekordbox":info.name} before syncing. Preview checks the destination. Sync saves a recoverable backup.</p>
     <p className="sync-note">Supported smart playlists stay dynamic. Other smart playlists export their saved tracks.</p>
    </div>
    <div className="sync-options"><h2>Options</h2><label className="sync-check"><input type="checkbox" checked={options.materialize_smartlists} onChange={e=>update("materialize_smartlists",e.target.checked)}/>All smartlists to playlists</label>{software==="engine_dj"?<label className="sync-check"><input type="checkbox" checked={options.engine_copy_audio} onChange={e=>update("engine_copy_audio",e.target.checked)}/>Copy audio to the target drive</label>:null}
     {(software==="serato4"||software==="pioneer_usb")?<label>Convert keys<select aria-label="Convert keys" value={options.key_notation} onChange={e=>update("key_notation",e.target.value as SyncOptions["key_notation"])}><option value="original">Original keys</option><option value="musical">Musical notation</option><option value="camelot">Camelot</option><option value="open_key">Open Key</option></select></label>:null}
     <label className="sync-check"><input type="checkbox" checked={options.nearest_color} onChange={e=>update("nearest_color",e.target.checked)}/>Use nearest supported colors</label>
     {software!=="pioneer_usb"?<label className="sync-check"><input type="checkbox" checked={options.skip_beatgrids} onChange={e=>update("skip_beatgrids",e.target.checked)}/>Keep existing beatgrids</label>:null}
     {(software==="engine_dj"||software==="serato4")?<label>Beatgrid locks<select aria-label="Beatgrid locks" value={options.beatgrid_lock} onChange={e=>update("beatgrid_lock",e.target.value as SyncOptions["beatgrid_lock"])}><option value="default">Default</option><option value="lock">Lock</option><option value="unlock">Unlock</option></select></label>:null}
     {(software==="engine_dj"||software==="pioneer_usb")?<label className="sync-check"><input type="checkbox" checked={options.add_album_art} onChange={e=>update("add_album_art",e.target.checked)}/>Add album art</label>:null}
     {software==="pioneer_usb"?<><label>Cue destination<select aria-label="Cue destination" value={options.usb_cue_destination} onChange={e=>update("usb_cue_destination",e.target.value as SyncOptions["usb_cue_destination"])}><option value="hot">Hot cues</option><option value="memory">Memory cues</option><option value="both">Hot and memory cues</option></select></label><label className="sync-check"><input type="checkbox" checked={options.usb_cleanup_unused} onChange={e=>update("usb_cleanup_unused",e.target.checked)}/>Clean up unused files</label><label>CDJ compatibility<select aria-label="CDJ compatibility" value={options.usb_cdj_compatibility} onChange={e=>update("usb_cdj_compatibility",e.target.value as SyncOptions["usb_cdj_compatibility"])}><option value="off">No detection</option><option value="detect">Detect incompatible audio</option><option value="auto_fix">Convert incompatible audio</option></select></label><label className="sync-check"><input type="checkbox" checked={options.extra_cue_markers} onChange={e=>update("extra_cue_markers",e.target.checked)}/>Extra cue markers</label></>:null}
     <details className="sync-mapping"><summary>Field mappings</summary><p>Choose which local value fills each destination field.</p>{fields.map(field=><label key={field}>{field}<select aria-label={`Map ${field}`} value={options.field_mappings[field]??field} onChange={e=>update("field_mappings",{...options.field_mappings,[field]:e.target.value as SyncTextField})}>{fields.map(source=><option key={source} value={source}>{source}</option>)}</select></label>)}</details>
    </div>
   </fieldset>
   <div className="sync-actions"><button className="secondary" disabled={!canPreview||!!busy} onClick={()=>void prepare()}>{busy==="preview"?<LoaderCircle className="spin" size={15}/>:<Search size={15}/>}Preview sync</button><button className="primary" disabled={!preview||!!busy} onClick={apply}>{busy==="apply"?<LoaderCircle className="spin" size={15}/>:<ArrowRightLeft size={15}/>}Sync to {info.name}</button>{preview?<button className="secondary" disabled={!!busy} onClick={()=>void discard(preview.id)}><Trash2 size={15}/>Discard preview</button>:null}</div>
   {preview?<section className="sync-preview" aria-label="Sync preview"><h2>{preview.tracks.toLocaleString()} tracks · {preview.playlists.toLocaleString()} playlists</h2><p>{preview.deleted_playlists?`${preview.deleted_playlists} owned playlists to remove · `:""}{preview.retained_external_playlists?`${preview.retained_external_playlists} matched playlists retained · `:""}{preview.files.length.toLocaleString()} files to update{preview.directories.some(d=>!d.before_manifest)?` · ${preview.directories.filter(d=>!d.before_manifest).length} folders to create`:""} · {size(preview.bytes_to_write)}</p><div>{preview.warnings.map((warning,index)=><p className="attention" key={index}>{warning}</p>)}</div><ul>{preview.renames?.map(rename=><li key={rename.from}><span>{rename.from} → {rename.to}</span><small>Rename</small></li>)}{preview.directories.filter(d=>!d.before_manifest||!d.after_exists).map(d=><li key={d.target}><span>{d.target}</span><small>{d.after_exists?"Create folder":"Remove folder"}</small></li>)}{preview.files.slice(0,100).map(file=><li key={file.target}><span>{file.target}</span><small>{file.after_sha256===null?"Remove":file.before_sha256===null?"Create":"Update"}</small></li>)}</ul>{preview.files.length>100?<p>{preview.files.length-100} more files in this preview.</p>:null}</section>:null}
   <section className="sync-history" aria-label="Sync history"><h2>Sync history</h2>{history.length?<ul>{history.map(entry=><li key={entry.id}><div><strong>{syncTargets.find(t=>t.id===entry.target?.software)?.name??"Sync preparation"}</strong><span>{entry.state??"interrupted"} · {entry.tracks} tracks{entry.created?` · ${new Date(entry.created).toLocaleString()}`:""}</span>{entry.error?<p role="status">{entry.error}</p>:null}</div>{entry.state==="prepared"?<button className="secondary" disabled={!!busy} onClick={()=>void discard(entry.id)}><Trash2 size={14}/>Discard preview</button>:null}{entry.needs_acknowledgement?<button className="secondary" disabled={!!busy} onClick={()=>void complete(entry.id)}>Finish sync</button>:null}{entry.target&&entry.state&&(["applied","applying","restoring"].includes(entry.state)||entry.needs_restore_acknowledgement)?<button className="secondary" disabled={!!busy} onClick={()=>void restore(entry.id)}><RotateCcw size={14}/>{entry.needs_restore_acknowledgement?"Finish restore":"Restore backup"}</button>:null}</li>)}</ul>:<p>Your previews, completed syncs and recoverable backups will appear here.</p>}</section>
  </div>
 </section>;
}
