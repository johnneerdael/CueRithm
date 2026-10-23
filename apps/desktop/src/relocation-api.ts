import {invoke} from "@tauri-apps/api/core";
import {open} from "@tauri-apps/plugin-dialog";
import {nativePortableBackup} from "./portable-backup-api";
import type {RelocationChoice,RelocationPlan,RelocationRequest,RelocationResult} from "./relocation-types";
export const nativeRelocation={
 roots:()=>invoke<{source_root:string;track_count:number}[]>("relocation_roots"),
 history:()=>invoke<{operation_id:string;created:number;applied:number;undone:boolean}[]>("relocation_history"),
 continue:(taskId:string,planId:string)=>invoke<RelocationPlan>("relocation_continue",{taskId,planId}),
 plan:(taskId:string,request:RelocationRequest)=>invoke<RelocationPlan>("relocation_plan",{taskId,request}),
 apply:(taskId:string,planId:string,choices:RelocationChoice[],backup:{enabled:boolean;destination:string|null})=>invoke<RelocationResult>("relocation_apply",{taskId,planId,choices,backup}),
 undo:(taskId:string,operationId:string)=>invoke<RelocationResult>("relocation_undo",{taskId,operationId}),
 chooseFile:async()=>{const value=await open({title:"Choose replacement audio file",multiple:false,filters:[{name:"Audio",extensions:["mp3","aac","adts","m4a","flac","wav","aiff","aif","ogg","opus","wma"]}]});return typeof value==="string"?value:null;},
 chooseFolder:nativePortableBackup.chooseFolder,cancel:nativePortableBackup.cancel,onProgress:nativePortableBackup.onProgress,
};
export type RelocationBridge=typeof nativeRelocation;
