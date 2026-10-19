import {useMemo,useState} from "react";
import {Plus,Minus,ArrowUp,ArrowDown,ListOrdered} from "lucide-react";
import type {PlaylistNode,PlaylistWorkspace,PlaylistEdit} from "../playlist-types";
import {playlistOptions} from "./PlaylistPicker";
export default function PlaylistMembershipControls({workspace,active,selected,manualOrder,busy,onEdit,onMove,onManualOrder}:{workspace:PlaylistWorkspace;active:PlaylistNode|null;selected:Set<string>;manualOrder:boolean;busy:boolean;onEdit:(edit:PlaylistEdit,revision:number)=>Promise<boolean>;onMove:(direction:-1|1)=>void;onManualOrder:()=>void}){
 const [destination,setDestination]=useState("");
 const ordinary=useMemo(()=>playlistOptions(workspace.tree.nodes).filter(p=>p.node.kind==="playlist"),[workspace.tree.nodes]);
 const canAdd=ordinary.some(p=>p.node.id===destination);
 if(!selected.size&&!(active?.kind==="playlist"&&!manualOrder))return null;
 return <div className="playlist-membership-toolbar">
 {selected.size?<><select aria-label="Destination playlist" value={destination} disabled={busy||!ordinary.length} onChange={e=>setDestination(e.target.value)}><option value="">Choose playlist…</option>{ordinary.map(p=><option key={p.node.id} value={p.node.id}>{p.path}</option>)}</select><button disabled={busy||!canAdd} onClick={()=>void onEdit({type:"add_members",id:destination,track_ids:[...selected]},workspace.tree.revision)}><Plus size={13}/>Add to playlist</button></>:null}
 {active?.kind==="playlist"?<>{selected.size?<button disabled={busy} onClick={()=>void onEdit({type:"remove_members",id:active.id,track_ids:[...selected]},workspace.tree.revision)}><Minus size={13}/>Remove from playlist</button>:null}{manualOrder&&selected.size?<div className="playlist-order-controls"><span>Order</span><button aria-label="Move selected tracks up" title="Move selected tracks up" disabled={busy} onClick={()=>onMove(-1)}><ArrowUp size={14}/></button><button aria-label="Move selected tracks down" title="Move selected tracks down" disabled={busy} onClick={()=>onMove(1)}><ArrowDown size={14}/></button></div>:!manualOrder?<button disabled={busy} onClick={onManualOrder}><ListOrdered size={14}/>Manual order</button>:null}</>:null}
 </div>;
}
