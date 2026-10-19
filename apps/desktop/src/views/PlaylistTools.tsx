import {useState} from "react";
import {ChevronLeft,Search,Save,LoaderCircle} from "lucide-react";
import type {Bridge} from "../types";
import type {PlaylistWorkspace,PlaylistToolRequest,PlaylistChange} from "../playlist-types";
import PlaylistPicker from "../components/PlaylistPicker";
import {folderOptions} from "../playlist-tree";
import {usePlaylistPreview} from "../use-playlist-preview";
type Tool="combine"|"snapshot"|"sort"|"number"|"prefix"|"uppercase";
export default function PlaylistTools({bridge,workspace,initialId,onApplied,onClose,onGenerate,applyPreview}:{bridge:Bridge;workspace:PlaylistWorkspace;initialId:string|null;onApplied:(change:PlaylistChange)=>Promise<void>;onClose:()=>void;onGenerate?:()=>void;applyPreview?:(id:string,revision:number)=>Promise<PlaylistChange>}){
 const initial=workspace.tree.nodes.find(n=>n.id===initialId&&!n.deleted);
 const [tool,setTool]=useState<Tool>("combine"),[selected,setSelected]=useState<string[]>(initial?[initial.id]:[]),[name,setName]=useState("");
 const [parent,setParent]=useState(initial?.kind==="folder"?initial.id:initial?.parent_id??""),[mode,setMode]=useState<"union"|"intersection"|"absent_from_all">("union");
 const [order,setOrder]=useState<"name_ascending"|"name_descending"|"track_count_descending">("name_ascending"),[allSiblings,setAllSiblings]=useState(true);
 const [start,setStart]=useState("1"),[zero,setZero]=useState(false),[replace,setReplace]=useState(false),[text,setText]=useState("");
 const [shown,setShown]=useState(200);
 const active=workspace.tree.nodes.filter(n=>!n.deleted),sources=tool==="sort"?active.filter(n=>n.parent_id===(parent||null)):active;
 const ids=selected.filter(id=>sources.some(n=>n.id===id));
 let request:PlaylistToolRequest|null=null;
 if(tool==="combine"&&ids.length&&name.trim())request={type:"combine",source_ids:ids,parent_id:parent||null,name,mode};
 if(tool==="snapshot"&&ids.length===1&&name.trim())request={type:"snapshot",source_id:ids[0],parent_id:parent||null,name};
 if(tool==="sort"&&(allSiblings||ids.length))request={type:"sort_siblings",parent_id:parent||null,ids:allSiblings?null:ids,order};
 if(tool==="number"&&ids.length&&/^\d+$/.test(start)&&Number(start)<=4294967295)request={type:"prefix",ids,prefix:{type:"number",start:Number(start),leading_zero:zero,replace_existing:replace}};
 if(tool==="prefix"&&ids.length&&text.length>0)request={type:"prefix",ids,prefix:{type:"text",text}};
 if(tool==="uppercase"&&ids.length)request={type:"uppercase",ids};
 const state=usePlaylistPreview({request,revision:workspace.tree.revision,prepare:bridge.previewPlaylistTool,apply:applyPreview??bridge.applyPlaylistTool,onApplied});
 const busy=state.busy;
 return <section className="playlist-tool-view"><header className="view-title"><div><button className="back-link" onClick={onClose}><ChevronLeft size={15}/>Back to library</button><h1>Playlist tools</h1><p>Preview combinations, snapshots, names and ordering.</p></div>{onGenerate?<button className="secondary" onClick={onGenerate}>Generate playlists</button>:null}</header>
 <div className="playlist-tool-body">{state.error?<div role="alert" className="error-banner">{state.error}</div>:null}
 <fieldset className="playlist-tool-controls" disabled={busy==="apply"}>
 <label>Tool<select aria-label="Playlist tool" value={tool} onChange={e=>{setTool(e.target.value as Tool);setShown(200);}}><option value="combine">Combine playlists</option><option value="snapshot">Save ordinary snapshot</option><option value="sort">Sort playlists</option><option value="number">Number playlists</option><option value="prefix">Add text prefix</option><option value="uppercase">Uppercase names</option></select></label>
 {tool==="combine"?<label>Combination<select aria-label="Combination mode" value={mode} onChange={e=>setMode(e.target.value as typeof mode)}><option value="union">Tracks in any selected playlist</option><option value="intersection">Tracks in every selected playlist</option><option value="absent_from_all">Active library tracks absent from all selected playlists</option></select></label>:null}
 {tool==="combine"||tool==="snapshot"?<label>Result name<input aria-label="Result name" maxLength={255} value={name} onChange={e=>setName(e.target.value)}/></label>:null}
 {tool==="combine"||tool==="snapshot"||tool==="sort"?<label>{tool==="sort"?"Folder to sort":"Destination folder"}<select aria-label={tool==="sort"?"Folder to sort":"Destination folder"} value={parent} onChange={e=>setParent(e.target.value)}><option value="">Your playlists</option>{folderOptions(active,null).map(n=><option key={n.id} value={n.id}>{n.label}</option>)}</select></label>:null}
 {tool==="sort"?<><label>Order<select aria-label="Playlist sort order" value={order} onChange={e=>setOrder(e.target.value as typeof order)}><option value="name_ascending">Name A–Z</option><option value="name_descending">Name Z–A</option><option value="track_count_descending">Track count, highest first</option></select></label><label className="playlist-check"><input type="checkbox" checked={allSiblings} onChange={e=>setAllSiblings(e.target.checked)}/>All sibling playlists</label></>:null}
 {tool==="number"?<><label>Starting number<input aria-label="Starting number" inputMode="numeric" value={start} onChange={e=>setStart(e.target.value)}/></label><label className="playlist-check"><input type="checkbox" checked={zero} onChange={e=>setZero(e.target.checked)}/>Leading zeroes</label><label className="playlist-check"><input type="checkbox" checked={replace} onChange={e=>setReplace(e.target.checked)}/>Replace existing number prefixes</label><small>Numbers follow playlist tree order.</small></>:null}
 {tool==="prefix"?<label>Text prefix<input aria-label="Text prefix" maxLength={255} value={text} onChange={e=>setText(e.target.value)}/></label>:null}
 {tool!=="sort"||!allSiblings?<PlaylistPicker nodes={sources} selected={ids} onChange={setSelected}/>:null}
 {tool==="snapshot"?<small>Select one playlist or folder. Dynamic contents are resolved into an ordinary playlist.</small>:null}
 </fieldset>
 <div className="playlist-preview-actions"><button className="secondary" disabled={!request||!!busy} onClick={()=>void state.previewNow()}>{busy==="preview"?<LoaderCircle size={14} className="spin"/>:<Search size={14}/>}Preview changes</button><button className="primary" disabled={!state.preview||!!busy} onClick={()=>void state.applyNow()}><Save size={14}/>{busy==="apply"?"Applying…":"Apply preview"}</button>{state.completed?<p role="status" className="playlist-apply-result">{state.completed.edit_id?"Playlist changes applied. Use sidebar Undo to revert.":"No changes were needed."}</p>:null}</div>
 {state.preview?<section className="playlist-result-preview" aria-label="Playlist changes preview"><h2>{state.preview.changes.length} playlist {state.preview.changes.length===1?"change":"changes"}</h2><p>Applying updates the entire preview. Audio files and analysis stay unchanged.</p><div>{state.preview.changes.slice(0,shown).map(item=><article key={item.id}><div><strong>{item.name}</strong>{item.previous_name&&item.previous_name!==item.name?<small>Previously {item.previous_name}</small>:null}</div><span>{item.track_count} tracks</span><small>{item.is_new?"New playlist":`Position ${item.position+1}`}</small></article>)}</div>{state.preview.changes.length>shown?<button className="secondary" onClick={()=>setShown(n=>n+200)}>Show more changes</button>:null}</section>:null}
 </div></section>;
}
