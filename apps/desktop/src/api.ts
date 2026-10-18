import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {getCurrentWebviewWindow} from "@tauri-apps/api/webviewWindow";
import {open} from "@tauri-apps/plugin-dialog";
import type {Bridge,PreparationEvent,PlaybackEvent} from "./types";
export const nativeBridge:Bridge={
  getState:()=>invoke("library_state"),
  getMetadataFields:()=>invoke("metadata_fields"),
  listTracks:query=>invoke("list_tracks",{query}),
  selectTrackIds:query=>invoke("select_track_ids",{query}),
  getTrack:trackId=>invoke("get_track",{trackId}),
  getArtwork:(trackId,sourceSha256)=>invoke("get_artwork",{trackId,sourceSha256}),
  getTaggingStorage:()=>invoke("tagging_storage"),
  getTaggingCatalog:()=>invoke("tagging_catalog"),
  saveSettings:settings=>invoke("save_settings",{settings}),
  enqueuePaths:paths=>invoke("enqueue_paths",{paths}),
  retryJob:jobId=>invoke("retry_job",{jobId}),
  reprepareTrack:trackId=>invoke("reprepare_track",{trackId}),
  archiveTrack:(trackId,archived)=>invoke("archive_track",{trackId,archived}),
  playerLoad:trackId=>invoke("player_load",{trackId}),
  playerPlay:()=>invoke("player_play"),
  playerPause:()=>invoke("player_pause"),
  playerSeek:seconds=>invoke("player_seek",{seconds}),
  playerVolume:volume=>invoke("player_volume",{volume}),
  playerStatus:()=>invoke("player_status"),
  getWaveform:(trackId,bins)=>invoke("get_waveform",{trackId,bins}),
  onPreparation:handler=>listen<PreparationEvent>("preparation",event=>handler(event.payload)),
  onPlayback:handler=>listen<PlaybackEvent>("playback",event=>handler(event.payload)),
  onDrop:(handler,active)=>getCurrentWebviewWindow().onDragDropEvent(event=>{
    active(event.payload.type==="enter"||event.payload.type==="over");
    if(event.payload.type==="drop"){active(false);handler(event.payload.paths);}
  }),
  chooseFiles:async()=>{
    const value=await open({multiple:true,filters:[{name:"Audio",extensions:["mp3","aac","m4a","alac","flac","wav","wave","aiff","aif","aifc","ogg","oga","opus","wma"]},{name:"All files",extensions:["*"]}]});
    return Array.isArray(value)?value:value?[value]:[];
  },
  chooseFolder:async()=>{const value=await open({directory:true,multiple:false});return typeof value==="string"?value:null;},
};
export function errorMessage(error:unknown):string {
  return typeof error==="object"&&error!==null&&"message" in error?String(error.message):String(error);
}
