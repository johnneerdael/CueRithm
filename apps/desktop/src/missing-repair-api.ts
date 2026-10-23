import {invoke} from "@tauri-apps/api/core";
import type {MissingPreview,MissingRepairEntry,MissingRepairHistory} from "./missing-repair-types";

// Same operation-ID contract as maintenance-api.ts: one fresh ID per long operation.
export const nativeMissingRepair={
  previewRemoval:(operationId:string,scanId:string,selected:string[])=>invoke<MissingPreview>("maintenance_preview_missing_removal",{operationId,scanId,selected}),
  applyRemoval:(operationId:string,id:string)=>invoke<MissingRepairEntry>("maintenance_apply_missing_removal",{operationId,id}),
  previewRelink:(operationId:string,scanId:string,selected:string[],locations:string[])=>invoke<MissingPreview>("maintenance_preview_missing_relink",{operationId,scanId,selected,locations}),
  applyRelink:(operationId:string,id:string)=>invoke<MissingRepairEntry>("maintenance_apply_missing_relink",{operationId,id}),
  history:()=>invoke<MissingRepairHistory[]>("maintenance_missing_repair_history"),
  undo:(operationId:string,id:string)=>invoke<MissingRepairEntry>("maintenance_undo_missing_repair",{operationId,id}),
  discardUndo:(operationId:string,id:string)=>invoke<MissingRepairHistory>("maintenance_discard_missing_repair_undo",{operationId,id}),
};
export type MissingRepairBridge=typeof nativeMissingRepair;
/** Used when a caller supplies its own maintenance bridge but no repair bridge: nothing to list, nothing to run. */
export const noMissingRepair:MissingRepairBridge={
  previewRemoval:async()=>{throw new Error("Removing records is not available");},
  applyRemoval:async()=>{throw new Error("Removing records is not available");},
  previewRelink:async()=>{throw new Error("Finding moved files is not available");},
  applyRelink:async()=>{throw new Error("Finding moved files is not available");},
  history:async()=>[],
  undo:async()=>{throw new Error("Undo is not available");},
  discardUndo:async()=>{throw new Error("Undo is not available");},
};
