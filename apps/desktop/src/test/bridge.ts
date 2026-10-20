import catalog from "./tagging-catalog.json";
import {vi} from "vitest";
import type {Bridge,Track,Settings,PreparationEvent,Job,Playback} from "../types";
export const track=(id:string):Track=>({metadata:{},id,source_sha256:"hash-"+id,display_path:"/music/"+id+".flac",title:"Track "+id,artist:"Artist",album:"",genre:"House",state:"ready",archived:false,key:"8A",bpm:128,energy:6,duration:240});
const defaults:Settings={watch_folders:[],stability_millis:2000,autotag_enabled:false,cleanup_enabled:false,engine:{
  config:{key_frame_size:4096,key_hop_size:1024,rhythm_frame_size:1024,rhythm_hop_size:128,min_bpm:60,max_bpm:200},sample_rate:11025,max_seconds:null,beat_model:null,mel_model:null,key_model:null,key_neural_model:null,source_profile_timing:true,structure_features:false,
  lexicon_config:{genre:null,drop_at_start:"Never",min_cue_length_beats:64,add_emergency_loop:false},
}};
export function fakeBridge(rows:Track[]) {
  let revision=0,signature="";
  const dataRevision=()=>{const next=JSON.stringify(bridge.rows);if(next!==signature){signature=next;revision++;}return revision;};
  let snapshot={revision:"settings",settings:JSON.parse(JSON.stringify(defaults)) as Settings,models:{}};
  let listener:((e:PreparationEvent)=>void)|null=null;
  let playback:Playback={track_id:null,duration_seconds:0,position_seconds:0,playing:false,volume:0.65,error:null};
  const bridge:Bridge&{rows:Track[];jobs:Job[];changed:(e:PreparationEvent)=>void}={
    discoverDjTargets:vi.fn(async()=>[]),
    previewDjSync:vi.fn(async request=>({deleted_playlists:0,retained_external_playlists:0,id:"sync",target:request.target,tracks:rows.length,playlists:0,bytes_to_write:1024,files:[],warnings:[],directories:[]})),
    discardDjSync:vi.fn(async()=>{}),
    applyDjSync:vi.fn(async id=>({id,state:"applied" as const,changes:[],directories:[]})),
    restoreDjSync:vi.fn(async id=>({id,state:"restored" as const,changes:[],directories:[]})),
    djSyncHistory:vi.fn(async()=>[]),
    getPlaylists:vi.fn(async()=>({tree:{revision:0,nodes:[]},statuses:[],latest_undo:null,service_error:null})),
    editPlaylist:vi.fn(async()=>({edit_id:"edit",revision:1,created_ids:[],affected_ids:[]})),
    undoPlaylist:vi.fn(async()=>({edit_id:"undo",revision:2,created_ids:[],affected_ids:[]})),
    getSmartCatalog:vi.fn(async()=>({time_zone:"UTC",catalog:{fields:{},tags:[],categories:{},playlists:{}},tags:[],categories:[],playlists:[],stamp:{input_revision:0,playlist_revision:0,preference_revision:0}})),
    resolvePlaylistDate:vi.fn(async day=>Date.parse(`${day}T00:00:00Z`)),
    previewSmartList:vi.fn(async()=>({id:"preview",track_ids:[],stamp:{input_revision:0,playlist_revision:0,preference_revision:0},created:1,expires:null})),
    saveSmartPlaylist:vi.fn(async()=>({edit_id:"smart",revision:1,created_ids:[],affected_ids:[]})),
    previewPlaylistTool:vi.fn(async()=>({id:"tool",stamp:{input_revision:0,playlist_revision:0,preference_revision:0},created:1,expires:null,changes:[]})),
    applyPlaylistTool:vi.fn(async()=>({edit_id:"tool",revision:1,created_ids:[],affected_ids:[]})),
    previewSmartGeneration:vi.fn(async()=>({id:"generation",stamp:{input_revision:0,playlist_revision:0,preference_revision:0},created:1,expires:null,folder_id:null,new_nodes:0,items:[]})),
    applySmartGeneration:vi.fn(async()=>({edit_id:"generation",revision:1,created_ids:[],affected_ids:[]})),
    getPlaylistPreferences:vi.fn(async()=>({revision:0,preferences:{time_zone:null,key_mixing_mode:"harmonic" as const}})),
    savePlaylistPreferences:vi.fn(async preferences=>({revision:1,preferences})),
    playlistMembers:vi.fn(async()=>[]),
    cleanupCatalog:vi.fn(async()=>({genres:[],artists:[],categories:[],unavailable_tracks:[]})),
    previewCleanup:vi.fn(async()=>({id:"plan",created:1,proposals:[],unchanged:[],failed:[]})),
    applyCleanup:vi.fn(async()=>({id:"batch",applied:[],unchanged:[],conflicted:[],failed:[]})),
    cleanupHistory:vi.fn(async()=>[]),undoCleanup:vi.fn(async()=>({id:"undo",applied:[],unchanged:[],conflicted:[],failed:[]})),
    onLibraryChange:vi.fn(async()=>()=>{}),
    getTaggingStorage:vi.fn(async()=>({original_files:0,original_bytes:0,pending_cleanup:0})),
    getTaggingCatalog:vi.fn(async()=>JSON.parse(JSON.stringify(catalog))),
    resetCatalogAccess:vi.fn(async()=>JSON.parse(JSON.stringify(catalog))),
    getMetadataFields:vi.fn(async()=>[]),
    rows,jobs:[],changed:e=>listener?.(e),
    selectTrackIds:vi.fn(async q=>bridge.rows.filter(r=>q.view==="archive"?r.archived:!r.archived&&(q.view==="incoming"?r.state==="incoming":r.state==="ready")).map(r=>r.id)),
    getState:vi.fn(async()=>({counts:{tracks:bridge.rows.filter(r=>r.state==="ready"&&!r.archived).length,incoming:bridge.rows.filter(r=>r.state==="incoming"&&!r.archived).length,archive:bridge.rows.filter(r=>r.archived).length},revision:dataRevision(),jobs:bridge.jobs,settings:snapshot,service:{watch_errors:[],last_error:null}})),
    listTracks:vi.fn(async q=>{const data=bridge.rows.filter(r=>q.view==="archive"?r.archived:!r.archived&&(q.view==="incoming"?r.state==="incoming":r.state==="ready"));return {rows:data,total:data.length,revision:dataRevision(),next_cursor:null};}),
    getArtwork:vi.fn(async()=>null),
    getTrack:vi.fn(async id=>({track:track(id),analysis:null,source_sha256:"hash",pcm_sha256:"pcm"})),
    saveSettings:vi.fn(async settings=>{snapshot={revision:"saved",settings,models:{}};return snapshot;}),
    enqueuePaths:vi.fn(async()=>[]),retryJob:vi.fn(async()=>{}),reprepareTrack:vi.fn(async()=>{}),archiveTrack:vi.fn(async()=>{}),
    playerLoad:vi.fn(async id=>{playback={...playback,track_id:id,duration_seconds:240};return {track:track(id),analysis:null,source_sha256:"hash",pcm_sha256:"pcm"};}),
    playerPlay:vi.fn(async()=>{playback={...playback,playing:true};}),playerPause:vi.fn(async()=>{playback={...playback,playing:false};}),
    playerSeek:vi.fn(async seconds=>{playback={...playback,position_seconds:seconds};}),playerVolume:vi.fn(async()=>{}),
    playerStatus:vi.fn(async()=>playback),
    getWaveform:vi.fn(async()=>({duration_seconds:240,bins:[]})),
    onPreparation:vi.fn(async handler=>{listener=handler;return ()=>{listener=null;};}),
    onPlayback:vi.fn(async()=>()=>{}),onDrop:vi.fn(async()=>()=>{}),
    chooseFiles:vi.fn(async()=>[]),chooseFolder:vi.fn(async()=>null),
  };
  return bridge;
}
