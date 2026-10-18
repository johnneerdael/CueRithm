import {useState,useEffect,useRef,useCallback} from "react";
import {X,FolderPlus,Search,LoaderCircle} from "lucide-react";
import type {Bridge} from "../types";
import type {PlaylistNode,PlaylistWorkspace,PlaylistKind,PlaylistChange,PlaylistEdit,SmartDefinition,SmartCatalog,SmartPreview} from "../playlist-types";
import {errorMessage} from "../api";
import {allowedOperators,initialRuleValue,validRuleValue} from "../smart-rules";
import SmartRuleEditor from "../components/SmartRuleEditor";
import {folderOptions} from "../playlist-tree";

export default function PlaylistEditor({bridge,workspace,node,parentId,onSaved,onClose}:{bridge:Bridge;workspace:PlaylistWorkspace;node:PlaylistNode|null;parentId:string|null;onSaved:(change:PlaylistChange)=>Promise<void>;onClose:()=>void}){
  const [name,setName]=useState(node?.name??"");
  const [kind,setKind]=useState<PlaylistKind>(node?.kind??"playlist");
  const [parent,setParent]=useState(node?.parent_id??parentId??"");
  const [definition,setDefinition]=useState<SmartDefinition>(node?.definition??{match_all:true,rules:[]});
  const [catalog,setCatalog]=useState<SmartCatalog|null>(null),[catalogBusy,setCatalogBusy]=useState(false);
  const [preview,setPreview]=useState<SmartPreview|null>(null),[previewBusy,setPreviewBusy]=useState(false),[pending,setPending]=useState(0);
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null);
  const revision=useRef(workspace.tree.revision),request=useRef(0),alive=useRef(true),initialized=useRef(!!node?.definition),catalogAttempted=useRef(false);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;request.current++;};},[]);
  const folders=folderOptions(workspace.tree.nodes,node?.id??null);
  const invalidate=useCallback(()=>{request.current++;setPreview(null);setPreviewBusy(false);},[]);
  const updateDefinition=(next:SmartDefinition)=>{invalidate();setError(null);setDefinition(next);};
  const loadCatalog=useCallback(async()=>{
    catalogAttempted.current=true;setCatalogBusy(true);setError(null);
    try{const next=await bridge.getSmartCatalog();if(!alive.current)return;setCatalog(next);
      if(!initialized.current){initialized.current=true;const field=next.catalog.fields.title?"title":Object.keys(next.catalog.fields).find(id=>!next.catalog.fields[id].unavailable_reason&&allowedOperators(id,next.catalog.fields[id].kind).length);if(field){const operator=allowedOperators(field,next.catalog.fields[field].kind)[0];setDefinition({match_all:true,rules:[{id:crypto.randomUUID(),field,operator,value:initialRuleValue(operator),or:false}]});}}
    }catch(e){if(alive.current)setError(errorMessage(e));}finally{if(alive.current)setCatalogBusy(false);}
  },[bridge]);
  useEffect(()=>{if(kind==="smartlist"&&!catalog&&!catalogBusy&&!catalogAttempted.current)void loadCatalog();},[kind,catalog,catalogBusy,loadCatalog]);
  useEffect(()=>{if(!preview?.expires)return;const timer=setTimeout(()=>{setPreview(null);setError("Preview expired. Preview matches again before saving.");},Math.max(0,preview.expires-Date.now()));return()=>clearTimeout(timer);},[preview]);
  const valid=!!catalog&&definition.rules.every(r=>!!catalog.catalog.fields[r.field]&&!catalog.catalog.fields[r.field].unavailable_reason&&allowedOperators(r.field,catalog.catalog.fields[r.field].kind).includes(r.operator)&&validRuleValue(r.value)&&(!r.operator.includes("RegExp")||(r.value.kind==="text"&&r.value.value.trim().length>0&&r.value.value.length<=100)));
  const previewMatches=async()=>{
    if(!valid||pending)return;const generation=++request.current;setPreviewBusy(true);setError(null);setPreview(null);
    try{if(workspace.tree.revision!==revision.current)throw Error("Playlists changed. Reopen the editor before saving.");const next=await bridge.previewSmartList(definition,{track_ids:null,playlist_id:node?.id??null});if(alive.current&&generation===request.current)setPreview(next);}
    catch(e){if(alive.current&&generation===request.current)setError(errorMessage(e));}finally{if(alive.current&&generation===request.current)setPreviewBusy(false);}
  };
  const datePending=useCallback((delta:number)=>{setPending(n=>Math.max(0,n+delta));invalidate();},[invalidate]);
  const ruleError=useCallback((message:string)=>{setError(message);invalidate();},[invalidate]);
  const submit=async(event:React.FormEvent)=>{
    event.preventDefault();if(!name.trim()||busy)return;setBusy(true);setError(null);
    try{
      if(workspace.tree.revision!==revision.current)throw Error("Playlists changed. Reopen the editor before saving.");
      let result:PlaylistChange;
      if(kind==="smartlist"){
        if(!preview||!valid||pending||previewBusy||preview.expires!==null&&preview.expires<=Date.now())throw Error("Preview current matches before saving.");
        result=await bridge.saveSmartPlaylist(preview.id,node?.id??null,name,parent||null,preview.stamp.playlist_revision);
      }else{
        let edit:PlaylistEdit;
        if(node){const edits:PlaylistEdit[]=[];if(name!==node.name)edits.push({type:"rename",id:node.id,name});if((parent||null)!==node.parent_id)edits.push({type:"move",id:node.id,parent_id:parent||null,position:workspace.tree.nodes.filter(n=>!n.deleted&&n.parent_id===(parent||null)).length});edit={type:"batch",edits};}
        else edit={type:"create",parent_id:parent||null,kind,name,definition:null};
        result=await bridge.editPlaylist(edit,revision.current);
      }
      await onSaved(result);
    }catch(e){setError(errorMessage(e));if(kind==="smartlist")invalidate();}finally{if(alive.current)setBusy(false);}
  };
  return <section className="playlist-editor" role="dialog" aria-labelledby="playlist-editor-title">
    <header className="view-title"><div><div className="eyebrow">PLAYLISTS</div><h1 id="playlist-editor-title">{node?"Edit playlist":"Create playlist"}</h1><p>{kind==="smartlist"?"Rules keep membership up to date automatically.":"Organize your local library."}</p></div><button className="icon-button" aria-label="Close playlist editor" onClick={onClose} disabled={busy}><X size={18}/></button></header>
    <form onSubmit={event=>void submit(event)} className="playlist-editor-form">
      {error?<div role="alert" className="error-banner">{error}</div>:null}
      <label>Playlist name<input autoFocus value={name} onChange={e=>setName(e.target.value)} maxLength={255} disabled={busy}/></label>
      <label>Playlist type<select value={kind} onChange={e=>{invalidate();setKind(e.target.value as PlaylistKind);}} disabled={!!node||busy}><option value="playlist">Playlist</option><option value="folder">Folder</option><option value="smartlist">Smart playlist</option></select></label>
      <label>Parent folder<select value={parent} onChange={e=>setParent(e.target.value)} disabled={busy}><option value="">Your playlists</option>{folders.map(n=><option key={n.id} value={n.id}>{n.label}</option>)}</select></label>
      {kind==="smartlist"?<>
        {catalogBusy?<p role="status"><LoaderCircle className="spin" size={15}/>Loading rule fields…</p>:catalog?<><SmartRuleEditor bridge={bridge} catalog={catalog} value={definition} onChange={updateDefinition} onPending={datePending} onError={ruleError}/><small className="muted">Calendar dates use {catalog.time_zone}.</small></>:<button type="button" className="secondary" onClick={()=>void loadCatalog()}>Reload rule fields</button>}
        <div className="smart-preview"><button type="button" className="secondary" disabled={!valid||pending>0||previewBusy||busy} onClick={()=>void previewMatches()}>{previewBusy?<LoaderCircle size={14} className="spin"/>:<Search size={14}/>}Preview matches</button><span role="status">{pending?"Resolving calendar date…":preview?`${preview.track_ids.length.toLocaleString()} matching ${preview.track_ids.length===1?"track":"tracks"}`:previewBusy?"Evaluating conditions…":"Preview current matches before saving."}</span></div>
      </>:null}
      <div className="playlist-editor-footer"><button type="button" className="secondary" onClick={onClose} disabled={busy}>Cancel</button><button className="primary" disabled={busy||!name.trim()||kind==="smartlist"&&(!preview||!valid||pending>0||previewBusy)}><FolderPlus size={15}/>{busy?"Saving…":node?"Save changes":"Create"}</button></div>
    </form>
  </section>;
}
