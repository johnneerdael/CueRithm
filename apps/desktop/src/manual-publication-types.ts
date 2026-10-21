import type {Publication,PublishedCue} from "./types";
export interface ManualPublicationGuard {track_id:string;source_sha256:string;entity_epoch:number;cue_revision_id:string}
export interface ManualPublicationSnapshot {guard:ManualPublicationGuard;library_revision:number;publication:Publication;duration_seconds:number}
export interface ManualPublicationRequest {guard:ManualPublicationGuard;expected_revision:number;grid:Publication["grid"];cues:PublishedCue[]}
export interface ManualPublicationChange {track_id:string;title?:string;before:Publication;after:Publication;preserved_manual:boolean}
export interface ManualPublicationPreview {id:string;library_revision:number;changes:ManualPublicationChange[]}
export interface ManualPublicationResult {id:string;track_ids:string[];library_revision:number}
export interface ManualPublicationHistory {id:string;state:string;created:number;can_undo:boolean}
export type CueMarker="start"|"drop"|"breakdown"|"second_drop"|"second_breakdown"|"lastbeat"|"emergency_loop"|"normal";
export interface CueTemplateRow {checked:boolean;markerType:CueMarker;name:string;cueType:"hotcue"|"loop_hot"|"loop_active"|"memory_cue"|"memory_cue_loop";color:string|null;label?:string;anchor?:CueMarker;position?:"before"|"after";beatsOffset?:number;loopDuration?:number}
export interface CueTemplate {overwrite:boolean;startCueBehavior?:"atFirstBeat"|"atExistingCue"|"atZero";useFirstCueAsStart?:boolean;breakdownMinBeats:number;keepCuePositions:boolean;enableCustomAnchors:boolean;dropAtStart:string;customAnchors:Record<string,string>;rows:CueTemplateRow[]}
export interface SavedCueTemplate {id:string;name:string;template:CueTemplate}
export interface TemplatePublicationRequest {template_id:string;guards:ManualPublicationGuard[];expected_revision:number;preserve_manual:boolean}
export interface ManualPublicationApi {
  snapshot:(trackId:string)=>Promise<ManualPublicationSnapshot>;
  preview:(request:ManualPublicationRequest)=>Promise<ManualPublicationPreview>;
  apply:(previewId:string,expectedRevision:number)=>Promise<ManualPublicationResult>;
  history:(trackId:string)=>Promise<ManualPublicationHistory[]>;
  undo:(editId:string,expectedRevision:number)=>Promise<ManualPublicationResult>;
  templates:()=>Promise<SavedCueTemplate[]>;
  saveTemplate:(id:string|null,name:string,template:CueTemplate,expectedRevision:number)=>Promise<SavedCueTemplate>;
  deleteTemplate:(id:string,expectedRevision:number)=>Promise<void>;
  previewTemplate:(request:TemplatePublicationRequest)=>Promise<ManualPublicationPreview>;
}
