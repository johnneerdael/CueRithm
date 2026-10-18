import {useEffect,useRef,useState} from "react";
import {createPortal} from "react-dom";
import type {PlaylistEdit,PlaylistNode,PlaylistWorkspace} from "../playlist-types";
import {folderOptions} from "../playlist-tree";
import {errorMessage} from "../api";

export default function PlaylistMoveDialog({node,workspace,onMove,onClose}:{node:PlaylistNode;workspace:PlaylistWorkspace;onMove:(edit:PlaylistEdit,revision:number)=>Promise<boolean>;onClose:()=>void}){
  const [parent,setParent]=useState(node.parent_id??""),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null);
  const dialog=useRef<HTMLElement>(null),revision=useRef(workspace.tree.revision),alive=useRef(true);
  useEffect(()=>{alive.current=true;const previous=document.activeElement;dialog.current?.querySelector<HTMLSelectElement>("select")?.focus();return()=>{alive.current=false;if(previous instanceof HTMLElement&&previous.isConnected)previous.focus();};},[]);
  return createPortal(<div className="playlist-dialog-backdrop"><section ref={dialog} role="dialog" aria-modal="true" aria-label={`Move ${node.name}`} className="playlist-move-dialog" onKeyDown={event=>{
    event.stopPropagation();if(event.key==="Escape"&&!busy){event.preventDefault();onClose();}
    if(event.key==="Tab"){const controls=[...dialog.current!.querySelectorAll<HTMLElement>("select:not(:disabled),button:not(:disabled)")];const first=controls[0],last=controls.at(-1);if(event.shiftKey&&document.activeElement===first){event.preventDefault();last?.focus();}else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first?.focus();}}
  }}><h2>Move {node.name}</h2><form onSubmit={event=>{event.preventDefault();if(busy)return;setBusy(true);setError(null);void(async()=>{
    try{if(workspace.tree.revision!==revision.current)throw Error("Playlists changed. Reopen Move before continuing.");const ok=await onMove({type:"move",id:node.id,parent_id:parent||null,position:workspace.tree.nodes.filter(n=>!n.deleted&&n.id!==node.id&&n.parent_id===(parent||null)).length},revision.current);if(ok&&alive.current)onClose();}
    catch(e){if(alive.current)setError(errorMessage(e));}finally{if(alive.current)setBusy(false);}
  })();}}>
    {error?<p role="alert">{error}</p>:null}
    <label>Destination folder<select disabled={busy} value={parent} onChange={e=>setParent(e.target.value)}><option value="">Your playlists</option>{folderOptions(workspace.tree.nodes,node.id).map(n=><option key={n.id} value={n.id}>{n.label}</option>)}</select></label>
    <div className="playlist-editor-footer"><button type="button" className="secondary" disabled={busy} onClick={onClose}>Cancel</button><button className="primary" disabled={busy||(parent||null)===node.parent_id}>{busy?"Moving…":"Move"}</button></div>
  </form></section></div>,document.body);
}
