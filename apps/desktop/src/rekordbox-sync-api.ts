import {invoke} from "@tauri-apps/api/core";
import type {RekordboxBridge} from "./rekordbox-sync-types";

export const nativeRekordboxBridge:RekordboxBridge={
 discover:()=>invoke("discover_rekordbox_targets"),
 preflight:(operationId,targetId)=>invoke("preflight_rekordbox_sync",{operationId,targetId}),
 preview:(operationId,request)=>invoke("preview_rekordbox_sync",{operationId,request}),
 apply:(operationId,id)=>invoke("apply_rekordbox_sync",{operationId,id}),
 restore:(operationId,id)=>invoke("restore_rekordbox_sync",{operationId,id}),
 recover:(operationId,id)=>invoke("recover_rekordbox_sync",{operationId,id}),
 discard:id=>invoke("discard_rekordbox_sync",{id}),
 history:()=>invoke("rekordbox_sync_history"),
 cancel:operationId=>invoke("maintenance_cancel",{operationId}),
};
