import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {getCurrentWebviewWindow} from "@tauri-apps/api/webviewWindow";
import {open} from "@tauri-apps/plugin-dialog";
import type {Bridge,PreparationEvent,PlaybackEvent,LibraryChangeEvent} from "./types";
export const nativeBridge:Bridge={
  discoverDjTargets:()=>invoke("discover_dj_targets"),
  previewDjSync:request=>invoke("preview_dj_sync",{request}),
  discardDjSync:id=>invoke("discard_dj_sync",{id}),
  applyDjSync:id=>invoke("apply_dj_sync",{id}),
  restoreDjSync:id=>invoke("restore_dj_sync",{id}),
  djSyncHistory:()=>invoke("dj_sync_history"),
  getPlaylists:()=>invoke("playlists_workspace"),
  editPlaylist:(edit,expectedRevision)=>invoke("edit_playlist",{edit,expectedRevision}),
  undoPlaylist:(editId,expectedRevision)=>invoke("undo_playlist",{editId,expectedRevision}),
  getSmartCatalog:()=>invoke("smart_field_catalog"),
  resolvePlaylistDate:(day,expectedRevision)=>invoke("resolve_playlist_calendar_day",{day,expectedRevision}),
  previewSmartList:(definition,scope)=>invoke("preview_smart_list",{definition,scope}),
  saveSmartPlaylist:(previewId,playlistId,name,parentId,expectedRevision)=>invoke("save_smart_playlist",{previewId,playlistId,name,parentId,expectedRevision}),
  previewPlaylistTool:request=>invoke("preview_playlist_tool",{request}),
  applyPlaylistTool:(previewId,expectedRevision)=>invoke("apply_playlist_tool",{previewId,expectedRevision}),
  previewSmartGeneration:request=>invoke("preview_smart_generation",{request}),
  applySmartGeneration:(previewId,expectedRevision)=>invoke("apply_smart_generation",{previewId,expectedRevision}),
  getPlaylistPreferences:()=>invoke("get_playlist_preferences"),
  savePlaylistPreferences:(preferences,expectedRevision)=>invoke("save_playlist_preferences",{preferences,expectedRevision}),
  playlistMembers:(playlistId,manual)=>invoke("playlist_members",{playlistId,manual}),
  cleanupCatalog:config=>invoke("cleanup_catalog",{config}),
  previewCleanup:(scope,config)=>invoke("preview_cleanup",{scope,config}),
  applyCleanup:(planId,proposalIds)=>invoke("apply_cleanup",{planId,proposalIds}),
  cleanupHistory:()=>invoke("cleanup_history"),
  undoCleanup:batchId=>invoke("undo_cleanup",{batchId}),
  onLibraryChange:handler=>listen<LibraryChangeEvent>("library-change",event=>handler(event.payload)),
  getState:()=>invoke("library_state"),
  getMetadataFields:()=>invoke("metadata_fields"),
  listTracks:query=>invoke("list_tracks",{query}),
  selectTrackIds:query=>invoke("select_track_ids",{query}),
  getTrack:trackId=>invoke("get_track",{trackId}),
  getArtwork:(trackId,sourceSha256)=>invoke("get_artwork",{trackId,sourceSha256}),
  getTaggingStorage:()=>invoke("tagging_storage"),
  getTaggingCatalog:()=>invoke("tagging_catalog"),
  resetCatalogAccess:()=>invoke("reset_catalog_access"),
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
