import {playbackBpm} from "./published-analysis";
import {useState,useEffect,useRef,useCallback} from "react";
import {Search,Plus,SlidersHorizontal,Inbox,Music2,LoaderCircle,RotateCcw,Archive,CheckCircle2,X,Upload,Sun,Moon,SunMoon} from "lucide-react";
import {useTheme} from "./theme";
import {nativeBridge,errorMessage} from "./api";
import type {Bridge,Track,TrackPage,View,Sort,LibraryState,Detail,Playback,Waveform,TrackQuery,WorkspaceView} from "./types";
import Sidebar from "./components/Sidebar";
import TrackTable from "./components/TrackTable";
import {FileNotFoundError} from "./components/RowPreview";
import PlayerPanel from "./components/PlayerPanel";
import Settings from "./views/Settings";
import Cleanup from "./views/Cleanup";
import Organize from "./views/Organize";
import Sync from "./views/Sync";
import Maintenance from "./views/Maintenance";
import CompatibleTracks from "./views/CompatibleTracks";
import TracklistMatcher from "./views/TracklistMatcher";
import PlaylistFlow from "./components/PlaylistFlow";
import LibraryBackup from "./views/LibraryBackup";
import {nativeLibraryBackup} from "./backup-api";
import type {LibraryBackupBridge} from "./backup-types";
import {manualPublicationApi} from "./manual-publication-api";
import type {ManualPublicationApi,ManualPublicationResult} from "./manual-publication-types";
import type {CompatibleBridge} from "./compatible-types";
import type {TracklistBridge} from "./tracklist-types";
import type {PlaylistWorkspace,PlaylistNode,PlaylistChange,PlaylistEdit} from "./playlist-types";
import PlaylistEditor from "./views/PlaylistEditor";
import PlaylistTools from "./views/PlaylistTools";
import PlaylistGenerator from "./views/PlaylistGenerator";
import PlaylistMembershipControls from "./components/PlaylistMembershipControls";
import {moveSelectedPlaylistSlots,insertPlaylistSelectionBefore} from "./playlist-order";
import type {PlaylistDragPayload,PlaylistDropTarget} from "./playlist-drag";
import {usePlaylistDrag} from "./use-playlist-drag";
import {descendantIds} from "./playlist-tree";
import MetadataEditor,{type MetadataEditorHandle} from "./components/MetadataEditor";
import MetadataPending,{hasDraftChanges} from "./components/MetadataPending";
import MatchReview from "./components/MatchReview";
import CuratePanel,{type CurateHandle} from "./components/CuratePanel";
import type {CurationCatalog,CurationFilter} from "./curation-types";
import CurationFilters from "./components/CurationFilters";
import type {MetadataDraft} from "./editor-types";

const idle:Playback={track_id:null,duration_seconds:0,position_seconds:0,playing:false,volume:.65,error:null};
function flag(key:string){try{return localStorage.getItem(key)==="1";}catch{return false;}}
function preference(key:string,fallback:number,min:number,max:number){try{const v=Number(localStorage.getItem(key));return v>=min&&v<=max?v:fallback;}catch{return fallback;}}
const isLibraryView=(value:WorkspaceView):value is View=>value==="tracks"||value==="incoming"||value==="archive";
export default function App({bridge=nativeBridge,compatibleBridge,tracklistBridge,backupBridge=nativeLibraryBackup,publicationApi=manualPublicationApi}:{bridge?:Bridge;compatibleBridge?:CompatibleBridge;tracklistBridge?:TracklistBridge;backupBridge?:LibraryBackupBridge;publicationApi?:ManualPublicationApi}){
  const [view,setViewState]=useState<WorkspaceView>("tracks");
  const [lastView,setLastView]=useState<View>("tracks");
  const currentView=useRef(view);currentView.current=view;
  const viewSession=useRef(0);
  const setView=useCallback((next:WorkspaceView)=>{
    if(currentView.current!==next){viewSession.current++;currentView.current=next;}
    setViewState(next);
  },[]);
  const [compatibleSourceId,setCompatibleSourceId]=useState<string|null>(null);
  const [showPlaylistFlow,setShowPlaylistFlow]=useState(false),[flowReload,setFlowReload]=useState(0);
  const [canonicalMembers,setCanonicalMembers]=useState<{playlistId:string;ids:string[];error:string|null}|null>(null);
  const [playlists,setPlaylists]=useState<PlaylistWorkspace|null>(null);
  const [playlistId,setPlaylistId]=useState<string|null>(null);
  const [playlistEditor,setPlaylistEditor]=useState<{node:PlaylistNode|null;parentId:string|null}|null>(null);
  const [playlistBusy,setPlaylistBusy]=useState(false);
  const [playlistPanel,setPlaylistPanel]=useState<"tools"|"generator"|null>(null);
  const dragDescendants=useRef<{nodes:PlaylistNode[];id:string;ids:Set<string>}|null>(null);
  const playlistPanelRef=useRef(playlistPanel);playlistPanelRef.current=playlistPanel;
  const playlistMutation=useRef(false),playlistEditorRef=useRef(playlistEditor);playlistEditorRef.current=playlistEditor;
  const [cleanupSelection,setCleanupSelection]=useState<string[]>([]);
  const [organizationSelection,setOrganizationSelection]=useState<string[]>([]);
  const [state,setState]=useState<LibraryState|null>(null);
  const [rows,setRows]=useState<Track[]>([]),[total,setTotal]=useState(0);
  const [search,setSearch]=useState(""),[sort,setSort]=useState<Sort>("title"),[descending,setDescending]=useState(false);
  const [selected,setSelected]=useState(new Set<string>());
  const [inspector,setInspector]=useState<{mode:"edit"|"review"|"pending"|"curate";order?:string[];pendingKind?:"metadata"|"curation";draftId?:string;draftTrackIds?:string[]}|null>(null),[draftCount,setDraftCount]=useState(0);
  const [curationCount,setCurationCount]=useState(0),[curationCatalog,setCurationCatalog]=useState<CurationCatalog|null>(null),[curationFilter,setCurationFilter]=useState<CurationFilter|null>(null);
  const curate=useRef<CurateHandle>(null);
  const inspectorRef=useRef(inspector);inspectorRef.current=inspector;
  const metadataEditor=useRef<MetadataEditorHandle>(null),inspectorInitiator=useRef<HTMLElement|null>(null);
  const [closingWindow,setClosingWindow]=useState(false),[exitRequestId,setExitRequestId]=useState<string|null>(null),closePending=useRef(false),exitIntent=useRef<{id:string;cancelled:boolean}|null>(null),exitHandled=useRef(new Set<string>()),exitSetup=useRef(Promise.resolve());
  const exiting=closingWindow||exitRequestId!==null;
  const [compactInspector,setCompactInspector]=useState(()=>window.matchMedia?.("(max-width:1250px)").matches??true);
  const [page,setPage]=useState<TrackPage|null>(null),[loading,setLoading]=useState(true);
  const [toast,setToast]=useState<string|null>(null),[bootError,setBootError]=useState<string|null>(null);
  const [playback,setPlayback]=useState<Playback>(idle),[detail,setDetail]=useState<Detail|null>(null),[wave,setWave]=useState<Waveform|null>(null),[loadingTrack,setLoadingTrack]=useState(false);
  const [dragging,setDragging]=useState(false);
  const {mode,cycle}=useTheme();
  const [collapsed,setCollapsed]=useState(()=>flag("sidebar-collapsed"));
  const [sideWidth,setSideWidth]=useState(()=>preference("sidebar-width",224,180,340));
  const [playerHeight,setPlayerHeight]=useState(()=>preference("player-height",270,200,420));
  useEffect(()=>{try{localStorage.setItem("sidebar-collapsed",collapsed?"1":"0");}catch{}},[collapsed]);
  const previewLoader=useCallback(async(id:string)=>{const detail=await bridge.getTrack(id);if(detail.file_available===false)throw new FileNotFoundError();const wave=await bridge.getWaveform(id,240);return {wave,detail};},[bridge]);
  const query=useRef<TrackQuery>({view:"tracks",search:"",sort:"title",descending:false,limit:200,cursor:null});
  const playlistRequest=useRef(0);
  const loadedScope=useRef("");
  const request=useRef(0),loadedRows=useRef<Track[]>([]),revision=useRef(-1),playRequest=useRef(0),loadedDetail=useRef<Detail|null>(null);
  const barSeekQueue=useRef<Promise<void>>(Promise.resolve());
  const loadingTrackRef=useRef(false);
  const selectedRef=useRef(selected);selectedRef.current=selected;
  const searchInput=useRef<HTMLInputElement>(null);
  const alive=useRef(true);
  const internalDrag=usePlaylistDrag({canDrop:(payload,target)=>canDropInternal(payload,target),onStart:payload=>{if(payload.kind==="tracks")setSelected(new Set(payload.ids));},onDrop:(payload,target)=>dropInternal(payload,target)});
  query.current={view:!isLibraryView(view)?lastView:view,search,sort,descending,limit:200,cursor:null,playlist_id:playlistId,...(curationFilter?{curation:curationFilter}:{})};
  const notify=useCallback((error:unknown)=>{const message=errorMessage(error);if(!message.includes("superseded"))setToast(message);},[]);
  const flushInspector=async()=>{if(inspectorRef.current?.mode==="edit")await metadataEditor.current?.flush();else if(inspectorRef.current?.mode==="curate")await curate.current?.flush();};
  const navigate=(action:()=>void|Promise<void>)=>{if(exitIntent.current||closePending.current)return;if(inspectorRef.current?.mode!=="edit"&&inspectorRef.current?.mode!=="curate"){inspectorRef.current=null;setInspector(null);try{const result=action();if(result)void result.catch(notify);}catch(error){notify(error);}return;}void (async()=>{await flushInspector();if(!alive.current||exitIntent.current||closePending.current)return;inspectorRef.current=null;setInspector(null);await action();})().catch(notify);};
  const openInspector=(mode:"edit"|"review"|"pending"|"curate",row?:Track)=>{
    if(exitIntent.current||closePending.current)return;
    if(!inspectorRef.current)inspectorInitiator.current=document.activeElement instanceof HTMLElement?document.activeElement:null;
    void (async()=>{await flushInspector();if(!alive.current)return;const q={...query.current},order=mode==="curate"?await bridge.selectTrackIds({...q,expected_revision:revision.current>=0?revision.current:undefined}):undefined;if(!alive.current||JSON.stringify(q)!==JSON.stringify(query.current))return;if(row&&(!selectedRef.current.has(row.id)||mode==="review"))setSelected(new Set([row.id]));const pinned=row?[row.id]:[...selectedRef.current],next={mode,...(order?{order:[...new Set([...order,...pinned])]}:{})};inspectorRef.current=next;setInspector(next);})().catch(notify);
  };
  const closeInspector=async()=>{await flushInspector();inspectorRef.current=null;setInspector(null);setTimeout(()=>{if(alive.current){const target=inspectorInitiator.current;target?.isConnected?target.focus():document.querySelector<HTMLElement>(".table-body")?.focus();}},0);};
  const openCurationTrack=(id:string)=>{void (async()=>{await flushInspector();if(!alive.current)return;const q={...query.current},order=await bridge.selectTrackIds({...q,expected_revision:revision.current>=0?revision.current:undefined});if(!alive.current||JSON.stringify(q)!==JSON.stringify(query.current))return;setSelected(new Set([id]));const next={mode:"curate" as const,order:order.includes(id)?order:[id,...order]};inspectorRef.current=next;setInspector(next);})().catch(notify);};
  const openCurationPending=async()=>{await flushInspector();const next={mode:"pending" as const,pendingKind:"curation" as const};inspectorRef.current=next;setInspector(next);};
  const openDraft=(draft:MetadataDraft)=>{setSelected(new Set(draft.track_ids));const next={mode:"edit" as const,draftId:draft.id,draftTrackIds:draft.track_ids};inspectorRef.current=next;setInspector(next);};
  useEffect(()=>{const active=inspectorRef.current;if(active?.draftId&&active.draftTrackIds&&(active.draftTrackIds.length!==selected.size||active.draftTrackIds.some(id=>!selected.has(id)))){const next={mode:active.mode};inspectorRef.current=next;setInspector(next);}},[selected]);
  useEffect(()=>{const media=window.matchMedia?.("(max-width:1250px)");if(!media)return;const changed=()=>setCompactInspector(media.matches);media.addEventListener("change",changed);return()=>media.removeEventListener("change",changed);},[]);
  useEffect(()=>{let active=true,request=0;const refreshDrafts=()=>{const generation=++request;void bridge.metadataDrafts().then(drafts=>{if(active&&generation===request)setDraftCount(drafts.filter(draft=>!["saved","discarded"].includes(draft.state)&&(hasDraftChanges(draft.patch)||!!draft.error)).length);}).catch(notify);};refreshDrafts();const listener=bridge.onMetadataEditChange(refreshDrafts);return()=>{active=false;void listener.then(dispose=>dispose());};},[bridge,notify]);
  useEffect(()=>{const listener=bridge.onCloseRequest(async()=>{closePending.current=true;setClosingWindow(true);await flushInspector();},error=>{closePending.current=false;setClosingWindow(false);notify(error);});return()=>{void listener.then(dispose=>dispose());};},[bridge,notify]);
  useEffect(()=>{
    let disposed=false,registered=false;
    const requested=(id:string)=>{
      if(disposed||exitHandled.current.has(id))return;
      exitHandled.current.add(id);const intent={id,cancelled:false};exitIntent.current=intent;setExitRequestId(id);
      void (async()=>{
        let completed=false;
        try{await flushInspector();if(exitIntent.current!==intent||intent.cancelled)return;await bridge.completeExit(id);completed=true;}
        catch(error){try{await bridge.cancelExit(id);}catch{/* Keep the original draft/exit error. */}if(!disposed)notify(error);}
        finally{if(!completed&&exitIntent.current===intent){exitIntent.current=null;if(!disposed)setExitRequestId(null);}}
      })();
    };
    const listener=bridge.onAppExitRequested(requested);
    exitSetup.current=exitSetup.current.catch(()=>{}).then(async()=>{await listener;if(disposed)return;const pending=await bridge.registerExitGuard();registered=true;if(!disposed&&pending)requested(pending);});
    void exitSetup.current.catch(error=>{if(!disposed)notify(error);});
    return()=>{disposed=true;void listener.then(dispose=>dispose()).catch(()=>{});exitSetup.current=exitSetup.current.catch(()=>{}).then(async()=>{if(registered)await bridge.unregisterExitGuard();});void exitSetup.current.catch(()=>{});};
  },[bridge,notify]);
  const cancelExit=async()=>{const intent=exitIntent.current;if(!intent)return;intent.cancelled=true;try{await bridge.cancelExit(intent.id);}catch(error){exitHandled.current.delete(intent.id);notify(error);}finally{if(exitIntent.current===intent){exitIntent.current=null;setExitRequestId(null);}}};

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
      if(alive.current&&generation===request.current&&JSON.stringify(q)===JSON.stringify(query.current)&&latest){loadedScope.current=JSON.stringify(q);loadedRows.current=out;setRows(out);setPage(latest);setTotal(latest.total);revision.current=latest.revision;}
    }catch(e){if(alive.current&&generation===request.current)notify(e);}
    finally{if(alive.current&&generation===request.current)setLoading(false);}
  },[bridge,notify]);
  const refreshPlaylists=useCallback(async()=>{
    const generation=++playlistRequest.current;
    try{const next=await bridge.getPlaylists();if(!alive.current)return;if(generation===playlistRequest.current)setPlaylists(previous=>!previous||next.tree.revision>=previous.tree.revision?next:previous);return next;}catch(e){if(alive.current&&generation===playlistRequest.current)notify(e);}
  },[bridge,notify]);
  const refresh=useCallback(async(force=false)=>{
    try{const next=await bridge.getState();if(!alive.current)return;setState(next);setBootError(null);const workspace=await refreshPlaylists();if(!alive.current||query.current.playlist_id&&(!workspace||!workspace.tree.nodes.some(n=>n.id===query.current.playlist_id&&!n.deleted)))return;if(force||revision.current!==next.revision)await fetchRows(false);}
    catch(e){if(alive.current)setBootError(errorMessage(e));}
  },[bridge,fetchRows,refreshPlaylists]);
  useEffect(()=>{
    if(bridge!==nativeBridge&&backupBridge===nativeLibraryBackup)return;
    let canceled=false;
    void backupBridge.pending().then(pending=>{if(!canceled&&pending?.blocked_reason)setView("backup");}).catch(error=>{if(!canceled)notify(error);});
    return()=>{canceled=true;};
  },[bridge,backupBridge,notify]);
  const publicationChanged=useCallback(async(result:ManualPublicationResult)=>{
    const current=loadedDetail.current,playGeneration=playRequest.current;
    if(current&&result.track_ids.includes(current.track.id)){
      const next=await bridge.getTrack(current.track.id);
      if(alive.current&&playGeneration===playRequest.current&&loadedDetail.current?.track.id===next.track.id){loadedDetail.current=next;setDetail(next);}
    }
    await refresh(true);
  },[bridge,refresh]);
  const addPaths=useCallback(async(paths:string[])=>{if(!paths.length)return;try{await bridge.enqueuePaths(paths);clearScopeRows();setPlaylistId(null);setPlaylistEditor(null);setPlaylistPanel(null);setSort(previous=>previous==="playlist_order"?"title":previous);setView("incoming");setLastView("incoming");await refresh(true);}catch(e){notify(e);}},[bridge,notify,refresh]);
  const load=useCallback(async(row:Track)=>{
    const generation=++playRequest.current;loadingTrackRef.current=true;setLoadingTrack(true);
    try{
      const next=await bridge.playerLoad(row.id);
      if(generation!==playRequest.current)return;
      loadedDetail.current=next;setDetail(next);setWave(null);
      void bridge.getWaveform(row.id,16384).then(data=>{if(generation===playRequest.current&&alive.current)setWave(data);}).catch(notify);
      await bridge.playerPlay();
      if(generation===playRequest.current)setPlayback(await bridge.playerStatus());
    }catch(e){notify(e);}finally{if(generation===playRequest.current){loadingTrackRef.current=false;setLoadingTrack(false);}}
  },[bridge,notify]);
  useEffect(()=>{
    alive.current=true;let lastPlayback=0;
    void refresh(true);
    void bridge.playerStatus().then(s=>{if(alive.current)setPlayback(s);}).catch(notify);
    const refreshDetail=(id:string)=>{
      if(loadedDetail.current?.track.id!==id)return;
      const generation=playRequest.current;
      void bridge.getTrack(id).then(next=>{
        if(alive.current&&generation===playRequest.current&&next.track.id===id&&loadedDetail.current?.track.id===id&&next.pcm_sha256&&next.pcm_sha256===loadedDetail.current.pcm_sha256){loadedDetail.current=next;setDetail(next);}
      }).catch(notify);
    };
    const prep=bridge.onPreparation(event=>{void refresh(false);if(event.state==="completed")refreshDetail(event.track_id);});
    const library=bridge.onLibraryChange(event=>{void refresh(false);if(!event.track_ids.length&&loadedDetail.current)refreshDetail(loadedDetail.current.track.id);else for(const id of event.track_ids)refreshDetail(id);});
    let accessRequest=0;
    const access=bridge.onCatalogAccessChange(()=>{const request=++accessRequest;void bridge.getState().then(next=>{if(alive.current&&request===accessRequest)setState(previous=>!previous||next.revision>=previous.revision?next:previous);}).catch(notify);});
    let curationRequest=0;const refreshCuration=()=>{const request=++curationRequest;void Promise.all([bridge.curationPendingPage(0,1),bridge.curationCatalog()]).then(([pending,catalog])=>{if(alive.current&&request===curationRequest){setCurationCount(pending.total);setCurationCatalog(catalog);}}).catch(notify);};refreshCuration();
    const curation=bridge.onCurationChange(()=>{refreshCuration();void refresh(false);});
    const player=bridge.onPlayback(event=>{if(alive.current&&event.sequence>lastPlayback){lastPlayback=event.sequence;setPlayback(event.status);}});
    const drop=bridge.onDrop(paths=>void addPaths(paths),setDragging);
    const timer=setInterval(()=>void refresh(false),1500);
    return()=>{alive.current=false;clearInterval(timer);void prep.then(unlisten=>unlisten());void library.then(unlisten=>unlisten());void access.then(unlisten=>unlisten());void curation.then(unlisten=>unlisten());void player.then(unlisten=>unlisten());void drop.then(unlisten=>unlisten());};
  },[bridge,refresh,addPaths,notify]);
  useEffect(()=>{if(!isLibraryView(view))return;void fetchRows(true);},[view,playlistId,search,sort,descending,curationFilter,fetchRows]);
  useEffect(()=>{setSelected(new Set());},[query.current.view,playlistId,search,curationFilter]);
  useEffect(()=>{if(!toast)return;const timer=setTimeout(()=>setToast(null),8000);return()=>clearTimeout(timer);},[toast]);
  const toggle=useCallback(async()=>{try{if(playback.playing)await bridge.playerPause();else await bridge.playerPlay();setPlayback(await bridge.playerStatus());}catch(e){notify(e);}},[playback.playing,bridge,notify]);
  const seek=useCallback(async(seconds:number)=>{try{await bridge.playerSeek(seconds);setPlayback(await bridge.playerStatus());}catch(e){notify(e);}},[bridge,notify]);
  const jumpBar=useCallback((direction:number)=>{
    const currentDetail=loadedDetail.current,bpm=playbackBpm(currentDetail?.analysis);
    if(!currentDetail||loadingTrackRef.current||!bpm||!Number.isFinite(bpm)||bpm<=0)return;
    const generation=playRequest.current,id=currentDetail.track.id;
    barSeekQueue.current=barSeekQueue.current.then(async()=>{
      if(generation!==playRequest.current)return;
      const current=await bridge.playerStatus();
      if(generation!==playRequest.current||current.track_id!==id)return;
      await bridge.playerSeek(Math.max(0,Math.min(current.duration_seconds,current.position_seconds+direction*240/bpm)));
      const next=await bridge.playerStatus();
      if(alive.current&&generation===playRequest.current&&next.track_id===id)setPlayback(next);
    }).catch(notify);
  },[bridge,notify]);
  const selectAll=useCallback(async()=>{
    const q={...query.current};
    try{const ids=await bridge.selectTrackIds({...q,expected_revision:revision.current>=0?revision.current:undefined});if(alive.current&&JSON.stringify(q)===JSON.stringify(query.current))setSelected(new Set(ids));}catch(e){notify(e);}
  },[bridge,notify]);
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{
      if(event.defaultPrevented||internalDrag.active.current||exitIntent.current||closePending.current)return;
      if((event.metaKey||event.ctrlKey)&&event.key==="f"){event.preventDefault();setPlaylistPanel(null);if(!isLibraryView(view))setView(lastView);searchInput.current?.focus();}
      if(playlistEditorRef.current||event.target instanceof Element&&event.target.closest("[role=dialog],[role=menu]"))return;
      if((event.metaKey||event.ctrlKey)&&!event.altKey&&!event.shiftKey&&event.key.toLowerCase()==="b"){event.preventDefault();setCollapsed(v=>!v);return;}
      if(event.target instanceof HTMLInputElement||event.target instanceof HTMLSelectElement||event.target instanceof HTMLTextAreaElement||(event.target instanceof HTMLElement&&event.target.isContentEditable))return;
      if(isLibraryView(view)&&!playlistPanelRef.current&&(event.metaKey||event.ctrlKey)&&!event.altKey&&event.key.toLowerCase()==="a"){event.preventDefault();void selectAll();return;}
      if(!event.metaKey&&!event.ctrlKey&&!event.altKey&&!event.shiftKey&&(event.key==="ArrowLeft"||event.key==="ArrowRight")&&playbackBpm(loadedDetail.current?.analysis)){event.preventDefault();jumpBar(event.key==="ArrowLeft"?-1:1);return;}
      if(event.code==="Space"&&detail&&!(event.target instanceof Element&&event.target.closest("button,a"))){event.preventDefault();void toggle();}
    };
    window.addEventListener("keydown",key);return()=>window.removeEventListener("keydown",key);
  },[toggle,detail,view,lastView,selectAll,jumpBar]);
  const resize=(event:React.PointerEvent,kind:"sidebar"|"player")=>{
    event.preventDefault();const x=event.clientX,y=event.clientY,sw=sideWidth,ph=playerHeight;
    const move=(e:PointerEvent)=>{if(kind==="sidebar")setSideWidth(Math.max(180,Math.min(340,sw+e.clientX-x)));else setPlayerHeight(Math.max(200,Math.min(420,ph+y-e.clientY)));};
    const end=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",end);};
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",end);
  };
  useEffect(()=>{try{localStorage.setItem("sidebar-width",String(sideWidth));localStorage.setItem("player-height",String(playerHeight));}catch{}},[sideWidth,playerHeight]);
  const clearScopeRows=()=>{request.current++;loadedRows.current=[];loadedScope.current="";setRows([]);setPage(null);setTotal(0);setLoading(true);setSelected(new Set());};
  const choosePlaylist=(node:PlaylistNode)=>navigate(()=>{clearScopeRows();setPlaylistEditor(null);setPlaylistPanel(null);setPlaylistId(node.id);setView("tracks");setLastView("tracks");setSearch("");setSort(node.kind==="playlist"?"playlist_order":"title");setDescending(false);});
  const onPlaylist=(id:string)=>{const node=playlists?.tree.nodes.find(n=>n.id===id&&!n.deleted);if(node)choosePlaylist(node);};
  const publishPlaylists=(next:PlaylistWorkspace)=>{if(!alive.current)return;playlistRequest.current++;setPlaylists(previous=>previous&&previous.tree.revision>next.tree.revision?previous:next);};
  const playlistSaved=async(change:PlaylistChange)=>{const next=await bridge.getPlaylists();if(!alive.current)return;publishPlaylists(next);if(playlistEditorRef.current===playlistEditor){setPlaylistEditor(null);const id=change.created_ids[0]??playlistEditor?.node?.id;const node=next.tree.nodes.find(n=>n.id===id&&!n.deleted);if(node)choosePlaylist(node);}void refresh(true);};
  const mutatePlaylist=async(operation:()=>Promise<PlaylistChange>)=>{
    if(playlistMutation.current)return false;playlistMutation.current=true;setPlaylistBusy(true);const scope=JSON.stringify(query.current);
    try{const change=await operation();const next=await bridge.getPlaylists();if(!alive.current)return false;publishPlaylists(next);if(change.created_ids.length&&scope===JSON.stringify(query.current)){const node=next.tree.nodes.find(n=>n.id===change.created_ids[0]&&!n.deleted);if(node)choosePlaylist(node);}void refresh(true);return true;}
    catch(e){if(alive.current){notify(e);await refreshPlaylists();}return false;}
    finally{playlistMutation.current=false;if(alive.current)setPlaylistBusy(false);}
  };
  const playlistAction=async(edit:PlaylistEdit,expectedRevision:number)=>{const scope=JSON.stringify(query.current),selection=selectedRef.current;const ok=await mutatePlaylist(()=>bridge.editPlaylist(edit,expectedRevision));if(ok&&edit.type==="remove_members"&&scope===JSON.stringify(query.current)&&selection===selectedRef.current)setSelected(new Set());return ok;};
  const reorderSelected=(direction:-1|1)=>{
    const q={...query.current},selection=selectedRef.current,expectedRevision=playlists?.tree.revision;
    if(!q.playlist_id||q.sort!=="playlist_order"||q.descending||!selection.size||expectedRevision===undefined)return;
    void mutatePlaylist(async()=>{const ids=await bridge.selectTrackIds(q),known=new Set(ids);if(JSON.stringify(q)!==JSON.stringify(query.current)||selection!==selectedRef.current||[...selection].some(id=>!known.has(id)))throw Error("Track list changed. Select tracks and try again.");const ordered=moveSelectedPlaylistSlots(ids,selection,direction);if(ordered.every((id,i)=>id===ids[i]))return {edit_id:null,revision:expectedRevision,created_ids:[],affected_ids:[]};return bridge.editPlaylist({type:"reorder_members",id:q.playlist_id!,track_ids:ordered},expectedRevision);});
  };
  const canDropInternal=(payload:PlaylistDragPayload,target:PlaylistDropTarget)=>{
    if(playlistBusy||!playlists)return false;const nodes=playlists.tree.nodes;
    if(payload.kind==="playlist"){
      if(!nodes.some(n=>n.id===payload.id&&!n.deleted))return false;if(target.kind==="root")return true;if(target.kind!=="playlist")return false;
      const dest=nodes.find(n=>n.id===target.id&&!n.deleted);if(!dest||dest.id===payload.id)return false;
      if(dragDescendants.current?.nodes!==nodes||dragDescendants.current.id!==payload.id)dragDescendants.current={nodes,id:payload.id,ids:descendantIds(nodes,payload.id)};
      return !dragDescendants.current.ids.has(dest.id);
    }
    if(target.kind==="playlist")return nodes.some(n=>n.id===target.id&&!n.deleted&&n.kind==="playlist");
    return target.kind==="track"&&query.current.sort==="playlist_order"&&!query.current.descending&&JSON.stringify(payload.query)===JSON.stringify(query.current)&&nodes.some(n=>n.id===query.current.playlist_id&&!n.deleted&&n.kind==="playlist");
  };
  const startTrackDrag=(row:Track,event:React.PointerEvent)=>{if(playlistBusy||!playlists||loadedScope.current!==JSON.stringify(query.current))return;const ids=selectedRef.current.has(row.id)?[...selectedRef.current]:[row.id];internalDrag.begin(event,{kind:"tracks",ids,query:{...query.current},revision:playlists.tree.revision});};
  const startPlaylistDrag=(id:string,event:React.PointerEvent)=>{if(!playlistBusy&&!playlistEditor&&playlists)internalDrag.begin(event,{kind:"playlist",id,revision:playlists.tree.revision});};
  const dropInternal=(payload:PlaylistDragPayload,target:PlaylistDropTarget)=>{
    if(!canDropInternal(payload,target))return;
    if(payload.kind==="playlist"){
      const nodes=playlists!.tree.nodes,dest=target.kind==="playlist"?nodes.find(n=>n.id===target.id):null,parent=dest?.kind==="folder"?dest.id:dest?.parent_id??null;
      const siblings=nodes.filter(n=>!n.deleted&&n.parent_id===parent&&n.id!==payload.id).sort((a,b)=>a.position-b.position||a.id.localeCompare(b.id));
      const position=dest&&dest.kind!=="folder"?siblings.findIndex(n=>n.id===dest.id):siblings.length;
      void playlistAction({type:"move",id:payload.id,parent_id:parent,position:Math.max(0,position)},payload.revision);return;
    }
    if(target.kind==="playlist"){void playlistAction({type:"add_members",id:target.id,track_ids:payload.ids},payload.revision);return;}
    if(target.kind!=="track"||!payload.query.playlist_id)return;
    void mutatePlaylist(async()=>{const q=payload.query,ids=await bridge.selectTrackIds(q),known=new Set(ids);if(JSON.stringify(q)!==JSON.stringify(query.current)||payload.ids.some(id=>!known.has(id)))throw Error("Track list changed. Drag the tracks again.");const ordered=insertPlaylistSelectionBefore(ids,new Set(payload.ids),target.id);if(ordered.every((id,i)=>id===ids[i]))return {edit_id:null,revision:payload.revision,created_ids:[],affected_ids:[]};return bridge.editPlaylist({type:"reorder_members",id:q.playlist_id!,track_ids:ordered},payload.revision);});
  };
  const undoPlaylist=()=>{if(playlists?.latest_undo)void mutatePlaylist(()=>bridge.undoPlaylist(playlists.latest_undo!,playlists.tree.revision));};
  const applyPlaylistPreview=async(operation:()=>Promise<PlaylistChange>)=>{if(playlistMutation.current)throw Error("Another playlist change is still being applied.");playlistMutation.current=true;setPlaylistBusy(true);try{return await operation();}finally{playlistMutation.current=false;if(alive.current)setPlaylistBusy(false);}};
  const playlistToolApplied=async(_change:PlaylistChange)=>{const next=await bridge.getPlaylists();if(!alive.current)return;publishPlaylists(next);void refresh(true);};
  const openPlaylistPanel=(panel:"tools"|"generator")=>navigate(()=>{setPlaylistEditor(null);setPlaylistPanel(panel);setView("tracks");setLastView("tracks");});
  const onView=(next:WorkspaceView)=>navigate(()=>{setPlaylistEditor(null);setPlaylistPanel(null);if(next==="tracks"||next==="incoming"||next==="archive"){clearScopeRows();setPlaylistId(null);if(sort==="playlist_order")setSort("title");}if(next==="cleanup")setCleanupSelection([...selectedRef.current]);if(next==="organize")setOrganizationSelection([...selectedRef.current]);if(next==="compatible")setCompatibleSourceId(selectedRef.current.values().next().value??loadedDetail.current?.track.id??null);if(isLibraryView(next))setLastView(next);setView(next);});
  useEffect(()=>{if(playlistId&&playlists&&!playlists.tree.nodes.some(n=>n.id===playlistId&&!n.deleted)){clearScopeRows();setPlaylistId(null);if(sort==="playlist_order")setSort("title");}},[playlistId,playlists,sort]);
  const onSelect=(row:Track,event:React.MouseEvent)=>{
    if(loadedScope.current!==JSON.stringify(query.current))return;
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
  const archive=async(row:Track)=>{try{await bridge.archiveTrack(row.id,!row.archived);setSelected(new Set());await refresh(true);}catch(e){notify(e);}};
  const archiveSelected=async(archived:boolean)=>{const ids=[...selectedRef.current];try{for(const id of ids)await bridge.archiveTrack(id,archived);setSelected(new Set());await refresh(true);}catch(e){notify(e);}};
  const active=state?.jobs.filter(j=>j.state==="running"||j.state==="queued")??[];
  const attention=state?.jobs.filter(j=>j.state==="attention")??[];
  const activePlaylist=playlists?.tree.nodes.find(n=>n.id===playlistId&&!n.deleted)??null;
  const activePlaylistStatus=playlists?.statuses.find(status=>status.playlist_id===playlistId);
  useEffect(()=>{
    let canceled=false;
    if(!showPlaylistFlow||activePlaylist?.kind!=="playlist"){setCanonicalMembers(null);return;}
    const id=activePlaylist.id;setCanonicalMembers(null);
    void bridge.playlistMembers(id,true).then(ids=>{if(!canceled)setCanonicalMembers({playlistId:id,ids,error:null});}).catch(error=>{if(!canceled)setCanonicalMembers({playlistId:id,ids:[],error:errorMessage(error)});});
    return()=>{canceled=true;};
  },[bridge,showPlaylistFlow,playlistId,activePlaylist?.kind,playlists?.tree.revision,state?.revision,flowReload]);
  const toolSaved=async(expectedView:WorkspaceView,session:number,change:PlaylistChange)=>{
    const next=await refreshPlaylists();if(!alive.current)return;
    if(currentView.current===expectedView&&viewSession.current===session){const node=next?.tree.nodes.find(n=>n.id===change.created_ids[0]&&!n.deleted);if(node)choosePlaylist(node);}
    await refresh(true);
  };
  const selectToolTrack=(row:Track)=>setSelected(new Set([row.id]));
  const renderedViewSession=viewSession.current;
  const libraryView=!isLibraryView(view)?lastView:view;
  const dragPayload=internalDrag.drag,dragLabel=dragPayload?.kind==="tracks"?`${dragPayload.ids.length} tracks`:dragPayload?playlists?.tree.nodes.find(n=>n.id===dragPayload.id)?.name??"Playlist":"";
  return <div className="app" data-collapsed={collapsed} data-internal-dragging={!!internalDrag.drag||undefined} onClickCapture={internalDrag.suppressClick} onDoubleClickCapture={internalDrag.suppressClick} style={{"--sidebar-width":sideWidth+"px","--player-height":playerHeight+"px"} as React.CSSProperties}>
    <Sidebar view={view} onView={onView} state={state} playlists={playlists} activePlaylistId={playlistId} onPlaylist={onPlaylist} onCreate={()=>navigate(()=>{setPlaylistPanel(null);setPlaylistEditor({node:null,parentId:activePlaylist?.kind==="folder"?activePlaylist.id:null});})} onEdit={node=>navigate(()=>{setPlaylistPanel(null);setPlaylistEditor({node,parentId:node.parent_id});})} onAction={playlistAction} onUndo={undoPlaylist} busy={playlistBusy||!!playlistEditor} playlistPanel={playlistPanel} onTools={()=>openPlaylistPanel("tools")} drag={internalDrag.drag} onPlaylistDragStart={startPlaylistDrag} collapsed={collapsed} onToggleCollapsed={()=>setCollapsed(v=>!v)}/>
    <div className="sidebar-resize" role="separator" aria-label="Resize sidebar" aria-orientation="vertical" onPointerDown={e=>resize(e,"sidebar")}/>
    <div className="main-column" inert={exiting}>
      <header className="app-header" data-tauri-drag-region>
        <div className="global-search"><Search size={16}/><input ref={searchInput} type="search" aria-label="Search library" placeholder="Search tracks, artists, genres…" value={search} onChange={e=>{setSearch(e.target.value);setPlaylistPanel(null);if(!isLibraryView(view))onView(lastView);}}/><kbd>⌘ F</kbd></div>
        <div className="processing-indicator">{!state?<span>{bootError?"Library unavailable":"Opening library…"}</span>:active.length?<><LoaderCircle size={14} className="spin"/><span>Preparing {active.length} {active.length===1?"track":"tracks"}</span></>:attention.length?<span className="attention">{attention.length} need attention</span>:<><i/><span>All up to date</span></>}</div>
        <button className="icon-button theme-toggle" aria-label={"Colour theme: "+mode} title={"Colour theme: "+mode+" (click to change)"} onClick={cycle}>{mode==="light"?<Sun size={17}/>:mode==="dark"?<Moon size={17}/>:<SunMoon size={17}/>}</button>
        <button className="primary add-tracks" onClick={()=>void bridge.chooseFiles().then(addPaths).catch(notify)}><Plus size={17}/>Add tracks</button>
      </header>
      <main className={inspector?"inspecting":undefined}>
        {bootError?<div role="alert" className="error-banner">{bootError}<button onClick={()=>void refresh(true)}>Try again</button></div>:null}
        {playlistEditor&&playlists?<PlaylistEditor bridge={bridge} workspace={playlists} node={playlistEditor.node} parentId={playlistEditor.parentId} onSaved={playlistSaved} onClose={()=>setPlaylistEditor(null)}/>:playlistPanel==="tools"&&playlists?<PlaylistTools bridge={bridge} workspace={playlists} initialId={playlistId} onApplied={playlistToolApplied} applyPreview={(id,rev)=>applyPlaylistPreview(()=>bridge.applyPlaylistTool(id,rev))} onClose={()=>{setPlaylistPanel(null);void refresh(true);}} onGenerate={()=>openPlaylistPanel("generator")}/>:playlistPanel==="generator"&&playlists?<PlaylistGenerator bridge={bridge} workspace={playlists} initialFolderId={activePlaylist?.kind==="folder"?activePlaylist.id:activePlaylist?.parent_id??null} onApplied={playlistToolApplied} applyPreview={(id,rev)=>applyPlaylistPreview(()=>bridge.applySmartGeneration(id,rev))} onClose={()=>{setPlaylistPanel(null);void refresh(true);}} onTools={()=>openPlaylistPanel("tools")}/>:view==="organize"&&state?<Organize bridge={bridge} revision={state.revision} onBack={()=>setView(lastView)} snapshot={state.settings} selectedIds={organizationSelection} playlistId={playlistId} workspace={playlists} onChanged={()=>void refresh(true)}/>:view==="compatible"?<CompatibleTracks bridge={bridge} compatibleBridge={compatibleBridge} initialSourceId={compatibleSourceId} onSelect={selectToolTrack} onPreview={load} onClose={()=>onView(lastView)} onSaved={change=>toolSaved("compatible",renderedViewSession,change)}/>:view==="tracklist"?<TracklistMatcher bridge={bridge} tracklistBridge={tracklistBridge} onSelect={selectToolTrack} onPreview={load} onClose={()=>onView(lastView)} onSaved={change=>toolSaved("tracklist",renderedViewSession,change)}/>:view==="backup"?<LibraryBackup bridge={backupBridge}/>:view==="maintenance"?<Maintenance revision={state?.revision} chooseFolder={bridge.chooseFolder} workspace={playlists} onChanged={()=>void refresh(true)}/>:view==="sync"?<Sync bridge={bridge} workspace={playlists} revision={state?.revision??0} onChanged={()=>void refresh(true)}/>:view==="settings"&&state?<Settings catalogRevision={state.revision} snapshot={state.settings} bridge={bridge} onSaved={()=>void refresh(true)} onBack={()=>onView(lastView)}/>:view==="cleanup"&&state?<Cleanup bridge={bridge} snapshot={state.settings} selectedIds={cleanupSelection} onChanged={()=>void refresh(true)}/>:<>
          <div className="view-title library-heading"><div><div className="eyebrow">LIBRARY</div><h1>{activePlaylist?activePlaylist.name:view==="incoming"?"Incoming":view==="archive"?"Archive":"Tracks"}<span className="heading-count">{activePlaylist?total:view==="incoming"?state?.counts.incoming:view==="archive"?state?.counts.archive:state?.counts.tracks}</span></h1><p>{view==="incoming"?"Automatic preparation. Ready tracks move into your library.":view==="archive"?"Archived tracks keep their analysis and cues.":"Key, BPM, energy and structural cues."}</p></div>
            <div className="view-actions">{activePlaylist?.kind==="playlist"?<button className="secondary" aria-pressed={showPlaylistFlow} onClick={()=>setShowPlaylistFlow(value=>!value)}>Playlist flow</button>:null}{attention.length&&view!=="incoming"?<button className="attention-link" onClick={()=>onView("incoming")}>{attention.length} need attention</button>:null}<button className="icon-button" aria-label="Preparation settings" title="Preparation settings" onClick={()=>onView("settings")}><SlidersHorizontal size={18}/></button></div>
          </div>

          {activePlaylistStatus&&activePlaylistStatus.state!=="ready"?<div className="playlist-evaluation-status" role={activePlaylistStatus.state==="error"?"alert":"status"}><strong>{activePlaylistStatus.error??(activePlaylistStatus.state==="pending"?"Evaluating playlist…":"Updating playlist…")}</strong><span>{activePlaylist?.kind==="folder"?(activePlaylistStatus.has_last_good_results?"Showing saved results from available smart playlists.":"Smart playlists have no saved results yet."):(activePlaylistStatus.has_last_good_results?"Showing the last successfully evaluated results.":"No saved results are available yet.")}</span></div>:null}
          {view==="incoming"&&state?.service.watch_errors.length?<div className="watch-warning">{state.service.watch_errors.join(" · ")}</div>:null}
          <div className="table-toolbar">{curationCatalog&&(curationCatalog.groups.length>0||curationFilter)?<CurationFilters catalog={curationCatalog} value={curationFilter} onApply={value=>navigate(()=>setCurationFilter(value))}/>:null}<span>{selected.size?selected.size+" selected":search?"Search results":"All "+(view==="incoming"?"incoming tracks":view==="archive"?"archived tracks":"tracks")}</span><div>
            {selected.size?<><button onClick={()=>openInspector("edit")}>Edit metadata</button><button disabled={selected.size!==1} onClick={()=>openInspector("review")}>Review match</button><button onClick={()=>openInspector("curate")}>Curate</button><button onClick={()=>onView("organize")}>Organize files</button><button onClick={()=>{void (async()=>{for(const id of selectedRef.current)await bridge.reprepareTrack(id);setSelected(new Set());await refresh(true);})().catch(notify);}}><RotateCcw size={13}/>Analyze again</button>{activePlaylist||view!=="archive"?<button onClick={()=>void archiveSelected(true)}><Archive size={13}/>Archive</button>:null}{activePlaylist||view==="archive"?<button onClick={()=>void archiveSelected(false)}><Archive size={13}/>Restore</button>:null}</>:<span className="sort-note">SORTED BY {sort.toUpperCase()}</span>}
            <button onClick={()=>openInspector("pending")}>Drafts and history{draftCount+curationCount?" · "+(draftCount+curationCount):""}</button>
          </div></div>
          {playlists?<PlaylistMembershipControls workspace={playlists} active={activePlaylist} selected={selected} manualOrder={sort==="playlist_order"&&!descending} busy={playlistBusy} onEdit={playlistAction} onMove={reorderSelected} onManualOrder={()=>{setSort("playlist_order");setDescending(false);}}/>:null}
          <div className={"library-workspace"+(inspector?" has-inspector":"")}><div className="library-table-column" hidden={!!inspector&&compactInspector}>
          {showPlaylistFlow&&activePlaylist?.kind==="playlist"?<>
            {canonicalMembers?.playlistId!==activePlaylist.id?<p role="status" className="playlist-evaluation-status">Loading playlist order…</p>:canonicalMembers.error?<div role="alert" className="error-banner">{canonicalMembers.error}<button onClick={()=>setFlowReload(value=>value+1)}>Retry playlist flow</button></div>:<PlaylistFlow key={activePlaylist.id} tracks={rows} orderedTrackIds={canonicalMembers.ids} totalCount={canonicalMembers.ids.length} selectedTrackId={selected.values().next().value??null} onSelect={id=>setSelected(new Set([id]))} onPlay={id=>{const row=rows.find(track=>track.id===id);if(row)void load(row);}}/>}
            {page?.next_cursor?<button className="secondary flow-load-more" disabled={loading} onClick={()=>void more()}>Load more track details</button>:null}
          </>:rows.length?<TrackTable curationCatalog={curationCatalog} suspended={!!inspector&&compactInspector} onInspect={(row,mode)=>openInspector(mode,row)} previewLoader={previewLoader} onTrackDragStart={startTrackDrag} metadataLoader={async()=>{const fields=await bridge.getMetadataFields();await refresh(true);return fields;}} artworkLoader={bridge.getArtwork} rows={rows} selected={selected} playingId={playback.playing?playback.track_id:null} onSelect={onSelect} onPlay={row=>void load(row)} sort={sort} descending={descending} onSort={next=>{if(sort===next)setDescending(v=>!v);else{setSort(next);setDescending(false);}}} view={libraryView} jobs={state?.jobs??[]} onRetry={id=>void bridge.retryJob(id).then(()=>refresh(true)).catch(notify)} onReprepare={id=>void reprepare(id)} onArchive={row=>void archive(row)} onMore={()=>void more()} hasMore={!!page?.next_cursor} loading={loading}/>:<div className="empty-state">{loading?<LoaderCircle size={34} className="spin"/>:view==="incoming"?<Inbox size={40}/>:<Music2 size={40}/>}<h2>{loading?(state?"Loading tracks…":"Opening your library…"):search?"No matching tracks":activePlaylist?"No tracks in this playlist":view==="incoming"?"Nothing incoming":view==="archive"?"Your archive is empty":"Your library starts here"}</h2><p>{search?"Try a different title, artist or genre.":activePlaylist?activePlaylist.kind==="smartlist"?"Tracks appear here when they match the playlist conditions.":activePlaylist.kind==="folder"?"Tracks from playlists in this folder appear here.":"Add tracks from your library to this playlist.":view==="incoming"?"Drop files here, or add music to a watch folder. Preparation runs automatically.":view==="archive"?"Archived tracks will be kept here with their analysis and cues.":"Drop music here. We’ll analyze it, generate cues and get it ready for your library."}</p>{!activePlaylist&&!search&&view!=="archive"?<button className="secondary" onClick={()=>void bridge.chooseFiles().then(addPaths).catch(notify)}><Plus size={16}/>Add audio files</button>:null}{!activePlaylist?<small>MP3 · AAC / ALAC · FLAC · WAV · AIFF · Ogg · Opus · WMA</small>:null}</div>}
          <footer className="library-footer"><span>{rows.length<total?rows.length+" of ":""}{total.toLocaleString()} tracks{selected.size?" · "+selected.size+" selected":""}</span><span><CheckCircle2 size={12}/>Offline audio analysis</span></footer>
          </div>{inspector?.mode==="curate"?<CuratePanel ref={curate} bridge={bridge} trackIds={[...selected]} order={inspector.order??[]} exiting={exiting} onNavigate={id=>{setSelected(new Set([id]));}} onClose={closeInspector} onPending={openCurationPending} onChanged={()=>void refresh(true)}/>:inspector?.mode==="edit"?<MetadataEditor ref={metadataEditor} exiting={exiting} bridge={bridge} trackIds={[...selected]} draftId={inspector.draftId} onClose={closeInspector} onPending={()=>openInspector("pending")} onChanged={()=>void refresh(true)}/>:inspector?.mode==="review"?<MatchReview bridge={bridge} trackId={selected.size===1?[...selected][0]:null} onClose={closeInspector} onPending={()=>openInspector("pending")} onChanged={()=>void refresh(true)}/>:inspector?.mode==="pending"?<MetadataPending bridge={bridge} initialTab={inspector.pendingKind} onOpenCuration={openCurationTrack} onOpenDraft={openDraft} onClose={closeInspector} onChanged={()=>void refresh(true)}/>:null}</div>
        </>}
      </main>
      <div className="player-resize" role="separator" aria-label="Resize player" aria-orientation="horizontal" onPointerDown={e=>resize(e,"player")}/>
      <PlayerPanel publicationApi={publicationApi} selectedTrackIds={[...selected]} onPublicationChanged={publicationChanged} artworkLoader={bridge.getArtwork} status={playback} detail={detail} data={wave} busy={loadingTrack} onToggle={()=>void toggle()} onPause={()=>void bridge.playerPause().then(()=>bridge.playerStatus()).then(setPlayback).catch(notify)} onSeek={seconds=>void seek(seconds)} onVolume={volume=>{setPlayback(p=>({...p,volume}));void bridge.playerVolume(volume).catch(notify);}}/>
    </div>
    {internalDrag.drag?<div ref={internalDrag.ghost} className="internal-drag-ghost" aria-hidden="true">{dragLabel}</div>:null}
    {exiting?<div className="app-exit-status" role="status"><LoaderCircle size={16} className="spin"/>{exitRequestId?"Saving the latest draft before quitting…":"Saving the latest draft before closing…"}{exitRequestId?<button className="secondary" onClick={()=>void cancelExit()}>Cancel quit</button>:null}</div>:null}
    {toast?<div role="alert" className="toast"><span>{toast}</span><button aria-label="Dismiss message" onClick={()=>setToast(null)}><X size={16}/></button></div>:null}
    {dragging?<div className="drop-overlay"><Upload size={42}/><h2>Drop to prepare</h2><p>Full analysis. Fresh cues. Ready for your library.</p></div>:null}
  </div>;
}
