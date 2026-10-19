import {vi} from "vitest";
import type {MaintenanceBridge} from "../maintenance-api";
import type {DuplicateReview,DuplicateScanReport,MaintenanceChange,MaintenanceProgress,MaintenanceStamp,ReviewGroup,ReviewTrack} from "../maintenance-types";

export const stamp:MaintenanceStamp={inputs:{input_revision:1,playlist_revision:1,preference_revision:1},maintenance_revision:1};
const report={created:1,stamp,stale:false,stale_reason:null};
export function candidate(id:string,extra:Partial<ReviewTrack["candidate"]>={}):ReviewTrack{
  return {candidate:{id,artist:"Artist",title:"Title "+id,duration:200,location:"/music/"+id+".mp3",comparison_path:"/music/"+id+".mp3",bitrate:320000,cue_count:2,present:true,ignored:false,...extra},extra1:""};
}
export function group(id="g1",ids=["a","b"],extra:Partial<ReviewGroup>={}):ReviewGroup{
  return {id,exact_audio:true,tracks:ids.map(i=>candidate(i)),keeper:ids[0],tags_from:null,state:"unlocked",ignored:[],changed_selection:false,...extra};
}
export function review(groups:ReviewGroup[]=[group()],revision=1):DuplicateReview{return {...report,id:"review",scan_id:"scan",revision,groups};}
export function duplicateScan(groups=1):DuplicateScanReport{
  return {...report,id:"scan",request:{scope:"full_library",playlist_ids:[],tolerance:"None",ignored_paths:[]},tracks:[],groups:Array.from({length:groups},(_,i)=>({id:"g"+(i+1),method:"fingerprint" as const,track_ids:["a","b"],suggested_keeper:"a",suggested_tags_from:null})),cache_hits:3,fingerprints_computed:5};
}

/** A MaintenanceBridge whose methods resolve to harmless empty results; tests override what they exercise. */
export function fakeMaintenance(){
  let progress:((e:MaintenanceProgress)=>void)|null=null,change:((e:MaintenanceChange)=>void)|null=null;
  const bridge={
    previewRetainedOriginals:vi.fn<MaintenanceBridge["previewRetainedOriginals"]>(async(_,includeUndoable)=>({id:"retained-preview",include_undoable:includeUndoable,files:[],size_bytes:0,undo_lost_writes:0})),
    disposeRetainedOriginals:vi.fn<MaintenanceBridge["disposeRetainedOriginals"]>(async()=>({id:"retained",preview_id:"retained-preview",file_count:0,size_bytes:0,undo_lost_writes:0,created:1,state:"committed",cleanup_pending:false,error:null})),
    retainedDisposalHistory:vi.fn<MaintenanceBridge["retainedDisposalHistory"]>(async()=>[]),
    retryRetainedDisposal:vi.fn<MaintenanceBridge["retryRetainedDisposal"]>(async()=>[]),
    previewBackupDisposal:vi.fn<MaintenanceBridge["previewBackupDisposal"]>(async()=>({id:"disposal-preview",operation_ids:[],files:[],size_bytes:0,restores_lost:[]})),
    disposeBackups:vi.fn(async()=>({id:"disposal",preview_id:"disposal-preview",operation_ids:[],file_count:0,size_bytes:0,state:"committed",cleanup_pending:false,error:null})),
    backupDisposalHistory:vi.fn<MaintenanceBridge["backupDisposalHistory"]>(async()=>[]),retryBackupDisposal:vi.fn<MaintenanceBridge["retryBackupDisposal"]>(async()=>[]),
    previewUnusedRemoval:vi.fn<MaintenanceBridge["previewUnusedRemoval"]>(async()=>({directories:[],id:"removal-preview",scan_id:"unused",stamp,files:[],size_bytes:0})),
    applyUnusedRemoval:vi.fn(async()=>({directories:[],id:"removal",preview_id:"removal-preview",stamp,files:[],size_bytes:0,restored:false,undo_of:null})),
    restoreUnusedRemoval:vi.fn(async()=>({directories:[],id:"restore",preview_id:"removal-preview",stamp,files:[],size_bytes:0,restored:true,undo_of:"removal"})),
    unusedRemovalHistory:vi.fn<MaintenanceBridge["unusedRemovalHistory"]>(async()=>[]),retryUnusedRecovery:vi.fn<MaintenanceBridge["retryUnusedRecovery"]>(async()=>[]),
    applyMerge:vi.fn(async()=>({id:"merge",preview_id:"merge-preview",stamp,archived_ids:["b"],restored_ids:[],affected_playlists:[],undo_of:null,updated_ids:["a"]})),
    undoMerge:vi.fn(async()=>({id:"undo",preview_id:"merge-preview",stamp,archived_ids:[],restored_ids:["b"],affected_playlists:[],undo_of:"merge",updated_ids:[]})),
    recovery:vi.fn<MaintenanceBridge["recovery"]>(async()=>[]),retryRecovery:vi.fn<MaintenanceBridge["retryRecovery"]>(async()=>[]),
    previewMerge:vi.fn<MaintenanceBridge["previewMerge"]>(async()=>({id:"merge-preview",review_id:"review",review_revision:1,created:1,stamp,plan:{groups:[]},playlists:[]})),
    mergeHistory:vi.fn<MaintenanceBridge["mergeHistory"]>(async()=>[]),
    scanDuplicates:vi.fn(async()=>duplicateScan()),duplicateScan:vi.fn(async()=>duplicateScan()),
    beginReview:vi.fn(async()=>review()),getReview:vi.fn(async()=>review()),editReview:vi.fn(async()=>review()),
    ignores:vi.fn<MaintenanceBridge["ignores"]>(async()=>({stamp,tracks:[]})),setIgnores:vi.fn<MaintenanceBridge["setIgnores"]>(async()=>({stamp,tracks:[]})),
    scanUnused:vi.fn<MaintenanceBridge["scanUnused"]>(async()=>({...report,id:"unused",request:{root:"/music",extension_mode:"all" as const,extensions:[]},files:[],size_bytes:0,complete:true,issues:[]})),
    unusedScan:vi.fn(),exportUnused:vi.fn(async()=>""),
    scanBroken:vi.fn<MaintenanceBridge["scanBroken"]>(async()=>({...report,id:"broken",request:{inspect_audio:false},decoder_identity:null,tracks:[]})),brokenScan:vi.fn(),
    cancel:vi.fn(async()=>true),
    onProgress:vi.fn(async(handler:(e:MaintenanceProgress)=>void)=>{progress=handler;return()=>{progress=null;};}),
    onChange:vi.fn(async(handler:(e:MaintenanceChange)=>void)=>{change=handler;return()=>{change=null;};}),
  } satisfies MaintenanceBridge;
  return Object.assign(bridge,{emitProgress:(e:MaintenanceProgress)=>progress?.(e),emitChange:()=>change?.({review_id:null,stamp})});
}
