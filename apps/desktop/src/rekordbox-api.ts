import {invoke} from "@tauri-apps/api/core";
import type {RekordboxPreflight,RekordboxPlaylist,ImportScope,NativeImportReport,NativeSyncPrepared,ApplyOutcome,NativeSyncRun} from "./rekordbox-types";

// Operations share the maintenance task slot: give each a fresh crypto.randomUUID() and cancel it with maintenance.cancel.
export const nativeRekordbox={
  preflight:()=>invoke<RekordboxPreflight>("rekordbox_preflight"),
  playlists:(operationId:string)=>invoke<RekordboxPlaylist[]>("rekordbox_playlists",{operationId}),
  importTracks:(operationId:string,scope:ImportScope)=>invoke<NativeImportReport>("rekordbox_import",{operationId,scope}),
  prepare:(operationId:string,addNewTracks=false)=>invoke<NativeSyncPrepared>("rekordbox_prepare",{operationId,addNewTracks}),
  apply:(operationId:string,addNewTracks=false)=>invoke<ApplyOutcome>("rekordbox_apply",{operationId,addNewTracks}),
  history:()=>invoke<NativeSyncRun[]>("rekordbox_history"),
};
export type RekordboxBridge=typeof nativeRekordbox;
