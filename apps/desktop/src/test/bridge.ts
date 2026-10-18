import {vi} from "vitest";
import type {Bridge,Track,Settings,PreparationEvent,Job,Playback} from "../types";
export const track=(id:string):Track=>({id,display_path:"/music/"+id+".flac",title:"Track "+id,artist:"Artist",album:"",genre:"House",state:"ready",key:"8A",bpm:128,energy:6,duration:240});
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
    rows,jobs:[],changed:e=>listener?.(e),
    getState:vi.fn(async()=>({counts:{tracks:bridge.rows.filter(r=>r.state==="ready").length,incoming:bridge.rows.filter(r=>r.state==="incoming").length,archive:0},revision:dataRevision(),jobs:bridge.jobs,settings:snapshot,service:{watch_errors:[],last_error:null}})),
    listTracks:vi.fn(async q=>{const data=bridge.rows.filter(r=>q.view==="incoming"?r.state==="incoming":r.state==="ready");return {rows:data,total:data.length,revision:dataRevision(),next_cursor:null};}),
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
    chooseFiles:vi.fn(async()=>[]),chooseFolder:vi.fn(async()=>null),chooseModel:vi.fn(async()=>null),
  };
  return bridge;
}
