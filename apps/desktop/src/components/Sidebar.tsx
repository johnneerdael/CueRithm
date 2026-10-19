import {ChevronRight,ChevronDown,Plus,Zap,Star,MoreHorizontal,Undo2,Trash2,ListMusic,Folder,Music2,PanelLeftClose,PanelLeftOpen} from "lucide-react";
import {useState,useEffect,useMemo,useCallback,useRef} from "react";
import type {PlaylistWorkspace,PlaylistNode,PlaylistEdit} from "../playlist-types";
import {playlistIndex,visiblePlaylistNodes} from "../playlist-tree";
import PlaylistMenu from "./PlaylistMenu";
import PlaylistMoveDialog from "./PlaylistMoveDialog";
import Glyph from "./Glyph";
import markLight from "../brand/cuerithm-mark-on-light.svg";
import markDark from "../brand/cuerithm-mark-on-dark.svg";
import type {PlaylistDragPayload} from "../playlist-drag";
import type {LibraryState,WorkspaceView} from "../types";
const peekDelay=140;
export default function Sidebar({view,onView,state,playlists,activePlaylistId,onPlaylist,onCreate,onEdit,onAction,onUndo,busy,playlistPanel,onTools,drag,onPlaylistDragStart,collapsed,onToggleCollapsed}:{view:WorkspaceView;onView:(view:WorkspaceView)=>void;state:LibraryState|null;playlists:PlaylistWorkspace|null;activePlaylistId:string|null;onPlaylist:(id:string)=>void;onCreate:()=>void;onEdit:(node:PlaylistNode)=>void;onAction:(edit:PlaylistEdit,revision:number)=>Promise<boolean>;onUndo:()=>void;busy:boolean;playlistPanel:"tools"|"generator"|null;onTools:()=>void;drag:PlaylistDragPayload|null;onPlaylistDragStart:(id:string,event:React.PointerEvent)=>void;collapsed:boolean;onToggleCollapsed:()=>void}){
  const [expanded,setExpanded]=useState(new Set<string>()),[removedOpen,setRemovedOpen]=useState(false);
  const [menu,setMenu]=useState<{id:string;revision:number;x:number;y:number}|null>(null),[moving,setMoving]=useState<PlaylistNode|null>(null);
  const nodes=playlists?.tree.nodes??[],revision=playlists?.tree.revision??0;
  const index=useMemo(()=>playlistIndex(nodes),[nodes]),visible=useMemo(()=>visiblePlaylistNodes(nodes,expanded),[nodes,expanded]);
  const statuses=new Map(playlists?.statuses.map(s=>[s.playlist_id,s]));
  const favorites=nodes.filter(n=>!n.deleted&&n.favorite_position!==null).sort((a,b)=>a.favorite_position!-b.favorite_position!);
  const removed=nodes.filter(n=>n.deleted&&(!n.parent_id||!index.byId.get(n.parent_id)?.deleted||index.byId.get(n.parent_id)?.deleted_by!==n.deleted_by));
  const closeMenu=useCallback(()=>setMenu(null),[]);
  useEffect(()=>{if(menu&&(menu.revision!==revision||index.byId.get(menu.id)?.deleted||!index.byId.has(menu.id)))setMenu(null);},[menu,revision,index]);
  const menuNode=menu?index.byId.get(menu.id):null;
  // A collapsed sidebar is a thin icon rail that expands over the content while hovered or focused.
  const [peek,setPeek]=useState(false);
  const timer=useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(()=>()=>clearTimeout(timer.current),[]);
  useEffect(()=>{if(!collapsed)setPeek(false);},[collapsed]);
  // Menus and dialogs render outside the sidebar, so keep it open while one is showing.
  const pinnedOpen=!!menu||!!moving;
  const enter=()=>{if(!collapsed)return;clearTimeout(timer.current);timer.current=setTimeout(()=>setPeek(true),peekDelay);};
  const leave=(event:React.SyntheticEvent)=>{if(pinnedOpen)return;if(event.type==="blur"&&event.currentTarget.contains((event as React.FocusEvent).relatedTarget as Node|null))return;clearTimeout(timer.current);setPeek(false);};
  const rail=collapsed&&!peek&&!pinnedOpen;
  const incoming=state?.counts.incoming??0;
  return <div className="sidebar-slot" data-collapsed={collapsed}>
  <aside className={"sidebar"+(rail?" rail":"")+(collapsed&&!rail?" peek":"")} onMouseEnter={enter} onMouseLeave={leave} onFocus={enter} onBlur={leave}>
    <div className="brand" data-tauri-drag-region><picture><img className="mark-light" src={markLight} alt=""/><img className="mark-dark" src={markDark} alt=""/></picture><span className="brand-name">CueRithm<small>MUSIC LIBRARY</small></span>
      <button className="rail-toggle" aria-label={collapsed?"Pin sidebar open":"Collapse sidebar"} aria-pressed={collapsed} title={collapsed?"Pin sidebar open (⌘B)":"Collapse to icons (⌘B)"} onClick={onToggleCollapsed}>{collapsed?<PanelLeftOpen size={16}/>:<PanelLeftClose size={16}/>}</button></div>
    <div className="sidebar-section">LIBRARY</div>
    <nav aria-label="Library">
      <button className={view==="tracks"&&!activePlaylistId&&!playlistPanel?"nav active":"nav"} title="Tracks" onClick={()=>onView("tracks")}><Glyph name="library"/><span className="nav-label">Tracks</span><span className="count">{state?.counts.tracks??0}</span></button>
      <button className={view==="incoming"?"nav active":"nav"} title="Incoming" onClick={()=>onView("incoming")}><Glyph name="incoming"/><span className="nav-label">Incoming</span><span className={incoming?"count accent":"count"}>{incoming}</span></button>
      <button className={view==="archive"?"nav active":"nav"} title="Archive" onClick={()=>onView("archive")}><Glyph name="folder"/><span className="nav-label">Archive</span><span className="count">{state?.counts.archive??0}</span></button>
    </nav>
    <div className="sidebar-section section-actions" data-playlist-drop-root>PLAYLISTS<div><button aria-label="Undo last playlist change" title="Undo last playlist change" onClick={onUndo} disabled={busy||!playlists?.latest_undo}><Undo2 size={14}/></button><button aria-label="Create playlist" onClick={onCreate} disabled={busy||!playlists}><Plus size={15}/></button></div></div>
    <div className="sidebar-playlists">
      {favorites.length?<nav className="playlist-favorites" aria-label="Favorite playlists">{favorites.map(n=><button key={n.id} aria-label={`Favorite playlist ${n.name}`} onClick={()=>onPlaylist(n.id)}><Star size={12}/><span>{n.name}</span></button>)}</nav>:null}
      {visible.length?<nav aria-label="Playlists"><ul className="playlist-tree">{visible.map(({node,depth})=>{const open=expanded.has(node.id),status=statuses.get(node.id);return <li key={node.id}>
        <div className={`playlist-row${activePlaylistId===node.id&&view==="tracks"?" active":""}`} data-playlist-drop-id={node.id} style={{paddingLeft:8+Math.min(depth,12)*14}} draggable={false} onDragStart={event=>event.preventDefault()} onPointerDown={event=>{if(!busy&&!(event.target as Element).closest(".playlist-actions,.playlist-expand"))onPlaylistDragStart(node.id,event);}}>
          {node.kind==="folder"?<button className="playlist-expand" aria-label={`${open?"Collapse":"Expand"} ${node.name}`} aria-expanded={open} onClick={()=>setExpanded(previous=>{const next=new Set(previous);if(next.has(node.id))next.delete(node.id);else next.add(node.id);return next;})}>{open?<ChevronDown size={12}/>:<ChevronRight size={12}/>}</button>:<span className="playlist-indent"/>}
          <button className="playlist-select" aria-label={`${node.kind==="folder"?"Folder":node.kind==="smartlist"?"Smart playlist":"Playlist"} ${node.name}`} onClick={()=>onPlaylist(node.id)} title={status?.error??node.name}>{node.kind==="folder"?<Folder size={15}/>:node.kind==="smartlist"?<Zap size={14}/>:<Music2 size={14}/>}<span>{node.name}</span>{status&&status.state!=="ready"?<i className={`playlist-state ${status.state}`} aria-label={status.state}/> :null}</button>
          <button className="playlist-actions" aria-label={`Playlist actions for ${node.name}`} aria-haspopup="menu" aria-expanded={menu?.id===node.id} disabled={busy} onClick={event=>{const rect=event.currentTarget.getBoundingClientRect();setMenu(menu?.id===node.id?null:{id:node.id,revision,x:Math.max(8,Math.min(rect.left,window.innerWidth-232)),y:Math.max(12,Math.min(rect.bottom+4,window.innerHeight-432))});}}><MoreHorizontal size={14}/></button>
        </div></li>;})}</ul></nav>:<div className="sidebar-note">Create a playlist, folder or smart list.</div>}
      {drag?.kind==="playlist"?<div className="playlist-root-drop" data-playlist-drop-root>Move to top level</div>:null}
      {removed.length?<div className="removed-playlists"><button aria-label="Removed playlists" aria-expanded={removedOpen} onClick={()=>setRemovedOpen(v=>!v)}><Trash2 size={12}/>Removed playlists<span>{removed.length}</span></button>{removedOpen?<><p>Restore playlists and their contents. Audio files stay in your library.</p><ul>{removed.map(n=><li key={n.id}><span title={n.name}>{n.name}</span><button aria-label={`Restore ${n.name}`} disabled={busy} onClick={()=>void onAction({type:"restore",id:n.id,parent_id:n.parent_id&&!index.byId.get(n.parent_id)?.deleted?n.parent_id:null,position:n.position},revision)}>Restore</button></li>)}</ul></>:null}</div>:null}
      {playlists?.service_error?<div role="alert" className="sidebar-note attention">{playlists.service_error}</div>:null}
    </div>
    <div className="sidebar-section">TOOLS</div>
    <button className={playlistPanel?"nav active":"nav"} title="Playlist tools" disabled={!playlists} onClick={onTools}><ListMusic size={18}/><span className="nav-label">Playlist tools</span></button>
    <button className={view==="cleanup"?"nav active":"nav"} title="Cleanup" onClick={()=>onView("cleanup")}><Glyph name="cleanup"/><span className="nav-label">Cleanup</span></button>
    <button className={view==="maintenance"?"nav active":"nav"} title="Maintenance" onClick={()=>onView("maintenance")}><Glyph name="warning"/><span className="nav-label">Maintenance</span></button>
    <button className="nav unavailable" disabled title="Direct rekordbox sync is not available in this build"><Glyph name="sync"/><span className="nav-label">Rekordbox</span></button>
    <div className="sidebar-bottom"><div className="offline-label"><i/><span>LOCAL LIBRARY</span></div><button className={view==="settings"?"nav active":"nav"} title="Settings" onClick={()=>onView("settings")}><Glyph name="settings"/><span className="nav-label">Settings</span></button><div className="build-label">CUERITHM <span>0.1</span></div></div>
    {menu&&menuNode&&!menuNode.deleted?<PlaylistMenu node={menuNode} nodes={nodes} x={menu.x} y={menu.y} onClose={closeMenu} onEdit={()=>onEdit(menuNode)} onMove={()=>setMoving(menuNode)} onAction={edit=>void onAction(edit,menu.revision)}/>:null}
    {moving&&playlists?<PlaylistMoveDialog node={moving} workspace={playlists} onMove={onAction} onClose={()=>setMoving(null)}/>:null}
  </aside>
  </div>;
}
