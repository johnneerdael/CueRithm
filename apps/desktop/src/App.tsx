import {useState,useEffect,useRef,useCallback} from "react";
import {Search,Plus,SlidersHorizontal,Inbox,Music2,LoaderCircle,RotateCcw,Archive,CheckCircle2,X,Upload} from "lucide-react";
import {nativeBridge,errorMessage} from "./api";
import type {Bridge,Track,TrackPage,View,Sort,LibraryState,Detail,Playback,Waveform,TrackQuery} from "./types";
import Sidebar from "./components/Sidebar";
import TrackTable from "./components/TrackTable";
import PlayerPanel from "./components/PlayerPanel";
import Settings from "./views/Settings";

const idle:Playback={track_id:null,duration_seconds:0,position_seconds:0,playing:false,volume:.65,error:null};
function preference(key:string,fallback:number,min:number,max:number){try{const v=Number(localStorage.getItem(key));return v>=min&&v<=max?v:fallback;}catch{return fallback;}}
export default function App({bridge=nativeBridge}:{bridge?:Bridge}){
  const [view,setView]=useState<View|"settings">("tracks");
  const [lastView,setLastView]=useState<View>("tracks");
  const [state,setState]=useState<LibraryState|null>(null);
  const [rows,setRows]=useState<Track[]>([]),[total,setTotal]=useState(0);
  const [search,setSearch]=useState(""),[sort,setSort]=useState<Sort>("title"),[descending,setDescending]=useState(false);
  const [selected,setSelected]=useState(new Set<string>());
  const [page,setPage]=useState<TrackPage|null>(null),[loading,setLoading]=useState(true);
  const [toast,setToast]=useState<string|null>(null),[bootError,setBootError]=useState<string|null>(null);
  const [playback,setPlayback]=useState<Playback>(idle),[detail,setDetail]=useState<Detail|null>(null),[wave,setWave]=useState<Waveform|null>(null),[loadingTrack,setLoadingTrack]=useState(false);
  const [dragging,setDragging]=useState(false);
  const [sideWidth,setSideWidth]=useState(()=>preference("sidebar-width",224,180,340));
  const [playerHeight,setPlayerHeight]=useState(()=>preference("player-height",270,200,420));
  const query=useRef<TrackQuery>({view:"tracks",search:"",sort:"title",descending:false,limit:200,cursor:null});
  const request=useRef(0),loadedRows=useRef<Track[]>([]),revision=useRef(-1),playRequest=useRef(0),loadedDetail=useRef<Detail|null>(null);
  const selectedRef=useRef(selected);selectedRef.current=selected;
  const searchInput=useRef<HTMLInputElement>(null);
  const alive=useRef(true);
  query.current={view:view==="settings"?lastView:view,search,sort,descending,limit:200,cursor:null};
  const notify=useCallback((error:unknown)=>{const message=errorMessage(error);if(!message.includes("superseded"))setToast(message);},[]);

  const fetchRows=useCallback(async(reset=false)=>{
    const generation=++request.current,q={...query.current};setLoading(true);
    try{
      let cursor:string|null=null,out:Track[]=[],latest:TrackPage|null=null;
      const target=reset?200:Math.max(200,loadedRows.current.length);
      for(let attempts=0;attempts<3;attempts++){
        try{
          out=[];cursor=null;
          do{latest=await bridge.listTracks({...q,cursor});out.push(...latest.rows);cursor=latest.next_cursor;}while(cursor&&out.length<target);
          break;
        }catch(e){if(!errorMessage(e).includes("Stale cursor")||attempts===2)throw e;}
      }
      if(alive.current&&generation===request.current&&latest){loadedRows.current=out;setRows(out);setPage(latest);setTotal(latest.total);revision.current=latest.revision;}
    }catch(e){if(alive.current&&generation===request.current)notify(e);}
    finally{if(alive.current&&generation===request.current)setLoading(false);}
  },[bridge,notify]);
  const refresh=useCallback(async(force=false)=>{
    try{const next=await bridge.getState();if(!alive.current)return;setState(next);setBootError(null);if(force||revision.current!==next.revision)await fetchRows(false);}
    catch(e){if(alive.current)setBootError(errorMessage(e));}
  },[bridge,fetchRows]);
  const addPaths=useCallback(async(paths:string[])=>{if(!paths.length)return;try{await bridge.enqueuePaths(paths);setView("incoming");setLastView("incoming");setSelected(new Set());await refresh(true);}catch(e){notify(e);}},[bridge,notify,refresh]);
  const load=useCallback(async(row:Track)=>{
    const generation=++playRequest.current;setLoadingTrack(true);
    try{
      const next=await bridge.playerLoad(row.id);
      if(generation!==playRequest.current)return;
      loadedDetail.current=next;setDetail(next);setWave(null);
      void bridge.getWaveform(row.id,16384).then(data=>{if(generation===playRequest.current&&alive.current)setWave(data);}).catch(notify);
      await bridge.playerPlay();
      if(generation===playRequest.current)setPlayback(await bridge.playerStatus());
    }catch(e){notify(e);}finally{if(generation===playRequest.current)setLoadingTrack(false);}
  },[bridge,notify]);
  useEffect(()=>{
    alive.current=true;let lastPlayback=0;
    void refresh(true);
    void bridge.playerStatus().then(s=>{if(alive.current)setPlayback(s);}).catch(notify);
    const prep=bridge.onPreparation(event=>{
      void refresh(false);
      if(event.state==="completed"&&loadedDetail.current?.track.id===event.track_id){
        const generation=playRequest.current;
        void bridge.getTrack(event.track_id).then(next=>{
          if(alive.current&&generation===playRequest.current&&next.track.id===event.track_id&&loadedDetail.current?.track.id===event.track_id&&next.pcm_sha256&&next.pcm_sha256===loadedDetail.current.pcm_sha256){loadedDetail.current=next;setDetail(next);}
        }).catch(notify);
      }
    });
    const player=bridge.onPlayback(event=>{if(alive.current&&event.sequence>lastPlayback){lastPlayback=event.sequence;setPlayback(event.status);}});
    const drop=bridge.onDrop(paths=>void addPaths(paths),setDragging);
    const timer=setInterval(()=>void refresh(false),1500);
    return()=>{alive.current=false;clearInterval(timer);void prep.then(unlisten=>unlisten());void player.then(unlisten=>unlisten());void drop.then(unlisten=>unlisten());};
  },[bridge,refresh,addPaths,notify]);
  useEffect(()=>{if(view==="settings")return;void fetchRows(true);},[view,search,sort,descending,fetchRows]);
  useEffect(()=>{setSelected(new Set());},[view,search]);
  useEffect(()=>{if(!toast)return;const timer=setTimeout(()=>setToast(null),8000);return()=>clearTimeout(timer);},[toast]);
  const toggle=useCallback(async()=>{try{if(playback.playing)await bridge.playerPause();else await bridge.playerPlay();setPlayback(await bridge.playerStatus());}catch(e){notify(e);}},[playback.playing,bridge,notify]);
  const seek=useCallback(async(seconds:number)=>{try{await bridge.playerSeek(seconds);setPlayback(await bridge.playerStatus());}catch(e){notify(e);}},[bridge,notify]);
  const selectAll=useCallback(async()=>{
    const q={...query.current};
    try{const ids=await bridge.selectTrackIds(q);if(alive.current&&q.view===query.current.view&&q.search===query.current.search)setSelected(new Set(ids));}catch(e){notify(e);}
  },[bridge,notify]);
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{
      if((event.metaKey||event.ctrlKey)&&event.key==="f"){event.preventDefault();if(view==="settings")setView(lastView);searchInput.current?.focus();}
      if(event.target instanceof HTMLInputElement||event.target instanceof HTMLSelectElement||event.target instanceof HTMLTextAreaElement||(event.target instanceof HTMLElement&&event.target.isContentEditable))return;
      if(view!=="settings"&&(event.metaKey||event.ctrlKey)&&!event.altKey&&event.key.toLowerCase()==="a"){event.preventDefault();void selectAll();return;}
      if(event.code==="Space"&&detail){event.preventDefault();void toggle();}
    };
    window.addEventListener("keydown",key);return()=>window.removeEventListener("keydown",key);
  },[toggle,detail,view,lastView,selectAll]);
  const resize=(event:React.PointerEvent,kind:"sidebar"|"player")=>{
    event.preventDefault();const x=event.clientX,y=event.clientY,sw=sideWidth,ph=playerHeight;
    const move=(e:PointerEvent)=>{if(kind==="sidebar")setSideWidth(Math.max(180,Math.min(340,sw+e.clientX-x)));else setPlayerHeight(Math.max(200,Math.min(420,ph+y-e.clientY)));};
    const end=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",end);};
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",end);
  };
  useEffect(()=>{try{localStorage.setItem("sidebar-width",String(sideWidth));localStorage.setItem("player-height",String(playerHeight));}catch{}},[sideWidth,playerHeight]);
  const onView=(next:View|"settings")=>{if(next!=="settings")setLastView(next);setView(next);};
  const onSelect=(row:Track,event:React.MouseEvent)=>{
    setSelected(previous=>{
      if(event.ctrlKey||event.metaKey){const next=new Set(previous);if(next.has(row.id))next.delete(row.id);else next.add(row.id);return next;}
      if(event.shiftKey&&previous.size){const start=rows.findIndex(r=>previous.has(r.id)),end=rows.findIndex(r=>r.id===row.id);return new Set(rows.slice(Math.max(0,Math.min(start,end)),Math.max(start,end)+1).map(r=>r.id));}
      return new Set([row.id]);
    });
  };
  const more=async()=>{
    if(!page?.next_cursor||loading)return;setLoading(true);const generation=request.current;
    try{const next=await bridge.listTracks({...query.current,cursor:page.next_cursor});if(generation===request.current){const combined=[...loadedRows.current,...next.rows];loadedRows.current=combined;setRows(combined);setPage(next);}}
    catch(e){if(errorMessage(e).includes("Stale cursor"))await fetchRows(false);else notify(e);}finally{setLoading(false);}
  };
  const reprepare=async(id:string)=>{try{await bridge.reprepareTrack(id);setSelected(new Set());await refresh(true);}catch(e){notify(e);}};
  const archive=async(id:string)=>{try{await bridge.archiveTrack(id,view!=="archive");setSelected(new Set());await refresh(true);}catch(e){notify(e);}};
  const active=state?.jobs.filter(j=>j.state==="running"||j.state==="queued")??[];
  const attention=state?.jobs.filter(j=>j.state==="attention")??[];
  const libraryView=view==="settings"?lastView:view;
  return <div className="app" style={{"--sidebar-width":sideWidth+"px","--player-height":playerHeight+"px"} as React.CSSProperties}>
    <Sidebar view={view} onView={onView} state={state}/>
    <div className="sidebar-resize" role="separator" aria-label="Resize sidebar" aria-orientation="vertical" onPointerDown={e=>resize(e,"sidebar")}/>
    <div className="main-column">
      <header className="app-header" data-tauri-drag-region>
        <div className="global-search"><Search size={16}/><input ref={searchInput} type="search" aria-label="Search library" placeholder="Search tracks, artists, genres…" value={search} onChange={e=>{setSearch(e.target.value);if(view==="settings")onView(lastView);}}/><kbd>⌘ F</kbd></div>
        <div className="processing-indicator">{!state?<span>{bootError?"Library unavailable":"Opening library…"}</span>:active.length?<><LoaderCircle size={14} className="spin"/><span>Preparing {active.length} {active.length===1?"track":"tracks"}</span></>:attention.length?<span className="attention">{attention.length} need attention</span>:<><i/><span>All up to date</span></>}</div>
        <button className="primary add-tracks" onClick={()=>void bridge.chooseFiles().then(addPaths).catch(notify)}><Plus size={17}/>Add tracks</button>
      </header>
      <main>
        {bootError?<div role="alert" className="error-banner">{bootError}<button onClick={()=>void refresh(true)}>Try again</button></div>:null}
        {view==="settings"&&state?<Settings snapshot={state.settings} bridge={bridge} onSaved={()=>void refresh(true)} onBack={()=>onView(lastView)}/>:<>
          <div className="view-title library-heading"><div><div className="eyebrow">LIBRARY</div><h1>{view==="incoming"?"Incoming":view==="archive"?"Archive":"Tracks"}<span className="heading-count">{view==="incoming"?state?.counts.incoming:view==="archive"?state?.counts.archive:state?.counts.tracks}</span></h1><p>{view==="incoming"?"Automatic preparation. Ready tracks move into your library.":view==="archive"?"Archived tracks keep their analysis and cues.":"Key, BPM, energy and structural cues."}</p></div>
            <div className="view-actions">{attention.length&&view!=="incoming"?<button className="attention-link" onClick={()=>onView("incoming")}>{attention.length} need attention</button>:null}<button className="icon-button" aria-label="Preparation settings" title="Preparation settings" onClick={()=>onView("settings")}><SlidersHorizontal size={18}/></button></div>
          </div>

          {view==="incoming"&&state?.service.watch_errors.length?<div className="watch-warning">{state.service.watch_errors.join(" · ")}</div>:null}
          <div className="table-toolbar"><span>{selected.size?selected.size+" selected":search?"Search results":"All "+(view==="incoming"?"incoming tracks":view==="archive"?"archived tracks":"tracks")}</span><div>
            {selected.size?<><button onClick={()=>{void (async()=>{for(const id of selectedRef.current)await bridge.reprepareTrack(id);setSelected(new Set());await refresh(true);})().catch(notify);}}><RotateCcw size={13}/>Analyze again</button><button onClick={()=>{void (async()=>{for(const id of selectedRef.current)await bridge.archiveTrack(id,view!=="archive");setSelected(new Set());await refresh(true);})().catch(notify);}}><Archive size={13}/>{view==="archive"?"Restore":"Archive"}</button></>:<span className="sort-note">SORTED BY {sort.toUpperCase()}</span>}
          </div></div>
          {rows.length?<TrackTable artworkLoader={bridge.getArtwork} rows={rows} selected={selected} playingId={playback.playing?playback.track_id:null} onSelect={onSelect} onPlay={row=>void load(row)} sort={sort} descending={descending} onSort={next=>{if(sort===next)setDescending(v=>!v);else{setSort(next);setDescending(false);}}} view={libraryView} jobs={state?.jobs??[]} onRetry={id=>void bridge.retryJob(id).then(()=>refresh(true)).catch(notify)} onReprepare={id=>void reprepare(id)} onArchive={id=>void archive(id)} onMore={()=>void more()} hasMore={!!page?.next_cursor} loading={loading}/>:<div className="empty-state">{loading&&!state?<LoaderCircle size={34} className="spin"/>:view==="incoming"?<Inbox size={40}/>:<Music2 size={40}/>}<h2>{loading&&!state?"Opening your library…":search?"No matching tracks":view==="incoming"?"Nothing incoming":view==="archive"?"Your archive is empty":"Your library starts here"}</h2><p>{search?"Try a different title, artist or genre.":view==="incoming"?"Drop files here, or add music to a watch folder. Preparation runs automatically.":view==="archive"?"Archived tracks will be kept here with their analysis and cues.":"Drop music here. We’ll analyze it, generate cues and get it ready for your library."}</p>{!search&&view!=="archive"?<button className="secondary" onClick={()=>void bridge.chooseFiles().then(addPaths).catch(notify)}><Plus size={16}/>Add audio files</button>:null}<small>MP3 · AAC / ALAC · FLAC · WAV · AIFF · Ogg · Opus · WMA</small></div>}
          <footer className="library-footer"><span>{rows.length<total?rows.length+" of ":""}{total.toLocaleString()} tracks{selected.size?" · "+selected.size+" selected":""}</span><span><CheckCircle2 size={12}/>Offline audio analysis</span></footer>
        </>}
      </main>
      <div className="player-resize" role="separator" aria-label="Resize player" aria-orientation="horizontal" onPointerDown={e=>resize(e,"player")}/>
      <PlayerPanel artworkLoader={bridge.getArtwork} status={playback} detail={detail} data={wave} busy={loadingTrack} onToggle={()=>void toggle()} onPause={()=>void bridge.playerPause().then(()=>bridge.playerStatus()).then(setPlayback).catch(notify)} onSeek={seconds=>void seek(seconds)} onVolume={volume=>{setPlayback(p=>({...p,volume}));void bridge.playerVolume(volume).catch(notify);}}/>
    </div>
    {toast?<div role="alert" className="toast"><span>{toast}</span><button aria-label="Dismiss message" onClick={()=>setToast(null)}><X size={16}/></button></div>:null}
    {dragging?<div className="drop-overlay"><Upload size={42}/><h2>Drop to prepare</h2><p>Full analysis. Fresh cues. Ready for your library.</p></div>:null}
  </div>;
}
