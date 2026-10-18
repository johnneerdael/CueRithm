import {useEffect,useRef} from "react";
import {createPortal} from "react-dom";
import {Copy,Pencil,Star,ArrowUp,ArrowDown,Folder,Trash2} from "lucide-react";
import type {PlaylistNode,PlaylistEdit} from "../playlist-types";

export default function PlaylistMenu({node,nodes,x,y,onClose,onEdit,onMove,onAction}:{node:PlaylistNode;nodes:PlaylistNode[];x:number;y:number;onClose:()=>void;onEdit:()=>void;onMove:()=>void;onAction:(edit:PlaylistEdit)=>void}){
  const menu=useRef<HTMLDivElement>(null);
  useEffect(()=>{const focused=document.activeElement;menu.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    const close=(event:MouseEvent)=>{if(!menu.current?.contains(event.target as Node))onClose();};
    document.addEventListener("mousedown",close);return()=>{document.removeEventListener("mousedown",close);if(focused instanceof HTMLElement&&focused.isConnected)focused.focus();};
  },[onClose]);
  const siblings=nodes.filter(n=>!n.deleted&&n.parent_id===node.parent_id).sort((a,b)=>a.position-b.position||a.id.localeCompare(b.id)),slot=siblings.findIndex(n=>n.id===node.id);
  const favorites=nodes.filter(n=>!n.deleted&&n.favorite_position!==null).sort((a,b)=>a.favorite_position!-b.favorite_position!),favorite=favorites.findIndex(n=>n.id===node.id);
  const action=(edit:PlaylistEdit)=>{onClose();onAction(edit);};
  const move=(position:number)=>action({type:"move",id:node.id,parent_id:node.parent_id,position});
  return createPortal(<div ref={menu} role="menu" aria-label={`Actions for ${node.name}`} className="context-menu playlist-context-menu" style={{left:x,top:y}} onKeyDown={event=>{
    event.stopPropagation();
    if(event.key==="Escape"||event.key==="Tab"){onClose();if(event.key==="Escape")event.preventDefault();return;}
    const buttons=[...menu.current!.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")],index=buttons.indexOf(document.activeElement as HTMLButtonElement);
    if(["ArrowDown","ArrowUp","Home","End"].includes(event.key)){event.preventDefault();buttons[event.key==="Home"?0:event.key==="End"?buttons.length-1:(index+(event.key==="ArrowDown"?1:-1)+buttons.length)%buttons.length]?.focus();}
  }}>
    <button role="menuitem" onClick={()=>{onClose();onEdit();}}><Pencil size={14}/>Edit</button>
    <button role="menuitem" onClick={()=>action({type:"duplicate",id:node.id,parent_id:node.parent_id,name:null})}><Copy size={14}/>Duplicate</button>
    <button role="menuitem" onClick={()=>action({type:"favorite",id:node.id,position:favorite<0?favorites.length:null})}><Star size={14}/>{favorite<0?"Add to favorites":"Remove from favorites"}</button>
    {favorite>=0?<><button role="menuitem" disabled={favorite===0} onClick={()=>action({type:"favorite",id:node.id,position:favorite-1})}><ArrowUp size={14}/>Move favorite up</button><button role="menuitem" disabled={favorite===favorites.length-1} onClick={()=>action({type:"favorite",id:node.id,position:favorite+1})}><ArrowDown size={14}/>Move favorite down</button></>:null}
    <div className="menu-divider"/>
    <button role="menuitem" disabled={slot<=0} onClick={()=>move(slot-1)}><ArrowUp size={14}/>Move up</button>
    <button role="menuitem" disabled={slot===siblings.length-1} onClick={()=>move(slot+1)}><ArrowDown size={14}/>Move down</button>
    <button role="menuitem" onClick={()=>{onClose();onMove();}}><Folder size={14}/>Move to folder…</button>
    <div className="menu-divider"/>
    <button role="menuitem" onClick={()=>action({type:"remove",id:node.id})}><Trash2 size={14}/>Remove</button>
  </div>,document.body);
}
