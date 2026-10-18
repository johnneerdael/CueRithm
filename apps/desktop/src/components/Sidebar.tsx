import {Music2,Inbox,Archive,Settings,Folder,ChevronRight,Plus,RefreshCw,Wand2,Radio} from "lucide-react";
import icon from "../assets/icon.svg";
import type {LibraryState,View} from "../types";
export default function Sidebar({view,onView,state}:{view:View|"settings";onView:(view:View|"settings")=>void;state:LibraryState|null}){
  return <aside className="sidebar">
    <div className="brand" data-tauri-drag-region><img src={icon} alt=""/><span>Cuerithm<small>MUSIC LIBRARY</small></span></div>
    <div className="sidebar-section">LIBRARY</div>
    <nav aria-label="Library">
      <button className={view==="tracks"?"nav active":"nav"} onClick={()=>onView("tracks")}><Music2 size={19}/>Tracks<span className="count">{state?.counts.tracks??0}</span></button>
      <button className={view==="incoming"?"nav active":"nav"} onClick={()=>onView("incoming")}><Inbox size={19}/>Incoming<span className={state?.counts.incoming?"count accent":"count"}>{state?.counts.incoming??0}</span></button>
      <button className={view==="archive"?"nav active":"nav"} onClick={()=>onView("archive")}><Archive size={18}/>Archive<span className="count">{state?.counts.archive??0}</span></button>
    </nav>
    <div className="sidebar-section section-actions">PLAYLISTS<button aria-label="Create playlist" disabled title="Playlist editing is not available in this build"><Plus size={15}/></button></div>
    <div className="playlist-empty"><Folder size={16}/><span>Your playlists</span><ChevronRight size={14}/></div>
    <div className="sidebar-note">Playlist tools will appear here when available.</div>
    <div className="sidebar-section">TOOLS</div>
    <button className="nav unavailable" disabled title="Cleanup is not available in this build"><Wand2 size={18}/>Cleanup</button>
    <button className="nav unavailable" disabled title="Direct rekordbox sync is not available in this build"><RefreshCw size={18}/>Rekordbox</button>
    <div className="sidebar-bottom">
      <div className="offline-label"><Radio size={13}/><span>LOCAL LIBRARY</span><i/></div>
      <button className={view==="settings"?"nav active":"nav"} onClick={()=>onView("settings")}><Settings size={18}/>Settings</button>
      <div className="build-label">CUERITHM <span>0.1</span></div>
    </div>
  </aside>;
}
