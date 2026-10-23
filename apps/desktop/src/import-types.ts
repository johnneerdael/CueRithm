/** Serialized shared-service contracts. The Rust fixture checks policy/state spelling. */
export type Json = null | boolean | number | string | Json[] | {[key:string]:Json};
export type SourceProvider = "rekordbox"|"serato3"|"serato4"|"engine_dj"|"traktor"|"virtual_dj"|"djay_pro"|"apple_music_xml"|"pioneer_usb"|"m3u"|"mixed_in_key11"|"dj_studio"|"mixxx";
export const sourceProviders:Record<SourceProvider,string>={rekordbox:"rekordbox 6/7",serato3:"Serato 3",serato4:"Serato 4",engine_dj:"Engine DJ",traktor:"Traktor",virtual_dj:"VirtualDJ",djay_pro:"djay Pro",apple_music_xml:"Apple Music XML",pioneer_usb:"Pioneer USB",m3u:"M3U / M3U8",mixed_in_key11:"Mixed In Key 11",dj_studio:"DJ.Studio",mixxx:"Mixxx"};
export type Presence<T>={presence:"absent"|"null"|"deleted"}|{presence:"empty"|"value";value:T};
export type ClockStatus="validated_profile"|"unresolved_native";
export interface FieldCapability {read:boolean;write:boolean;lossless:boolean;evidence:"d0"|"d1"|"d2"|"d3"|"d4"|"d5"|"d6";reason:string|null}
export interface ImportIssue {code:string;message:string;record_id?:string|null;[key:string]:Json|undefined}
export interface SourceProfile {provider:SourceProvider;profile_id:string;schema_fingerprint:string;origin:Record<string,string>;fields:Record<string,FieldCapability>;issues:ImportIssue[]}
export type FieldGroup="descriptive_tags"|"key"|"bpm"|"beatgrid"|"hot_cues"|"markers"|"memory_cues"|"main_cues"|"load_cues"|"loops"|"ratings"|"color"|"history"|"energy"|"artwork"|"custom_tags"|"collections";
export const defaultFields:FieldGroup[]=["descriptive_tags","key","bpm","beatgrid","hot_cues","markers","memory_cues","main_cues","load_cues","loops","ratings","color","energy","artwork","custom_tags","collections"];
export type RelocationPolicy={mode:"off"}|{mode:"rename_in_place";rule:string}|{mode:"move"|"copy";root_id:string;rule:string};
export interface ImportPolicy {version:number;selected_fields:FieldGroup[];descriptive_fields:string[]|null;metadata_enrichment:{enabled:boolean;providers:string[];fields:string[];credential_refs:Record<string,string>};cleanup:{enabled:boolean;rules:string[]};analysis_fields:Partial<Record<"key"|"bpm"|"beatgrid"|"energy","fill_missing"|"replace_selected">>;cue_generation:"off"|"generate";replace_hotcues:boolean;write_audio_tags:string[];relocation:RelocationPolicy;capture_file_tags:boolean;include_history:boolean;include_queue:boolean;allow_content_matching:boolean}
export function defaultImportPolicy():ImportPolicy {return {version:1,selected_fields:[...defaultFields],descriptive_fields:null,metadata_enrichment:{enabled:false,providers:[],fields:[],credential_refs:{}},cleanup:{enabled:false,rules:[]},analysis_fields:{},cue_generation:"off",replace_hotcues:false,write_audio_tags:[],relocation:{mode:"off"},capture_file_tags:false,include_history:false,include_queue:false,allow_content_matching:false};}
export function processingRequested(value:ImportPolicy):boolean {return value.metadata_enrichment.enabled||value.cleanup.enabled||Object.keys(value.analysis_fields).length>0||value.cue_generation!=="off"||value.write_audio_tags.length>0||value.relocation.mode!=="off";}
export interface ImportSourceCandidate {provider:SourceProvider;display_location:string;profile_hints:string[];read_state:string;acceptance:string;source_id:string|null}
export interface NativeImportSelection {selection_id:string;provider:SourceProvider;display_location:string;artifacts:string[];artifact_count:number}
export type SourceBindingChoice={choice:"automatic"|"new_library"}|{choice:"replica";source_id:string};
export interface ImportInspectRequest {selection_id:string;binding:SourceBindingChoice;capture_file_tags:boolean;include_history:boolean;include_queue:boolean;path_mappings:{source_prefix:string;root_id:string}[]}
export interface ImportInspection {inspection_id:string;source_id:string;profile:SourceProfile;header:{provider:SourceProvider;library_instance_id:string;product_version:string;completeness:"whole_library"|"selected_scope"|"partial_with_errors";[key:string]:Json};counts:{tracks:number;collections:number;memberships:number;issues:number}}
export interface TrackObservation {external_id:string;locations:{availability:string;original?:string|null;resolved?:string|null;[key:string]:Json|undefined};metadata:Record<string,Presence<Json>>;cached_tags:Record<string,Presence<Json>>;key?:{raw:string|null;camelot:string|null;[key:string]:Json}|null;bpm?:{value:number|null;[key:string]:Json}|null;energy:Presence<Json>;grid:Json;cues:Json[];extensions:Record<string,Json>;issues:ImportIssue[];[key:string]:unknown}
export interface CollectionObservation {external_id:string;parent_id:string|null;kind:string;name:string;occurrences:{external_occurrence_id:string;track_external_id:string;position:number|null}[];[key:string]:unknown}
export type ObservationRecord={record_type:"track";record:TrackObservation}|{record_type:"collection";record:CollectionObservation}|{record_type:"membership";record:{collection_id:string;occurrence:Json}}|{record_type:"issue";record:ImportIssue};
export interface PageQuery {limit?:number;cursor?:string|null}
export interface ImportPageQuery extends PageQuery {kind?:"track"|"collection"|"membership"|"issue"}
export interface ImportPage {records:ObservationRecord[];total:number;next_cursor:string|null}
export type RecordSelection={selection:"all"}|{selection:"ids";ids:string[]};
export type ImportDecision={choice:"keep_local"|"use_source"}|{choice:"custom";value:Json};
export interface ImportConflictDecision {external_id:string;field:string;decision:ImportDecision}
export interface ImportCandidateChoice {external_id:string;field:string;candidate_id:string}
export interface ImportPreviewRequest {inspection_id:string;tracks:RecordSelection;collections:RecordSelection;policy:ImportPolicy;target_items:Record<string,string>;decisions:ImportConflictDecision[];candidate_choices?:ImportCandidateChoice[]}
export interface ImportPreview {preview_id:string;tracks_created:number;fields_changed:number;baselines_changed:number;collections_changed:number;conflicts:number;blocked:number;processing_requested:boolean}
export interface ImportFieldEffect {field:string;source:Json;local:Json;proposed:Json;conflict:boolean;blocked_reason:string|null}
export interface ImportPreviewEffect {external_id:string;item_id:string;kind:string;fields:ImportFieldEffect[]}
export interface ImportPreviewPage {effects:ImportPreviewEffect[];total:number;next_cursor:string|null}
export interface ImportApplyResult {operation_id:string;tracks_created:number;fields_changed:number;collections_changed:number;processing_requested:boolean}
export interface ImportOperation {operation_id:string;source_id:string;state:string;created:number;updated:number;policy:ImportPolicy;result:ImportApplyResult;can_recover:boolean;can_undo:boolean;can_retry:boolean;can_review:boolean;stages:Record<string,number>;issues:{item_id:string;stage:string;state:string;message:string}[]}
export interface ImportOperationQuery extends PageQuery {source_id?:string|null}
export interface ImportOperationPage {records:ImportOperation[];total:number;next_cursor:string|null}
export interface ImportEffectsPreview {preview_id:string;operation_id:string;ready:number;blocked:number;unchanged:number}
export interface ImportEffect {item_id:string;stage:string;outcome:string;reason:string|null;changes:Json}
export interface ImportEffectsPage {records:ImportEffect[];next_cursor:string|null}
export interface ImportRecoveryResult {recovered:number;recovery_required:number;errors:string[]}
export interface ImportUndoResult {undone:number;blocked:number;errors:string[]}
export interface ImportedItemDetail {id:string;active_fields:Record<string,Json>;provenance:Record<string,Json>;observations:{source_id:string;external_id:string;snapshot_id:string;track:TrackObservation}[];availability:string;analysis_state:string;playback_allowed:boolean}
export interface ImportedCollectionPage {id:string;ordered:boolean;kind:string;rows:{occurrence_id:string;item_id:string|null;source_track_id:string;source_position:number|null;position:number;track:import("./types").Track|null}[];total:number;next_cursor:string|null}
export interface ImportProcessingSummary {processed:number;reused:number;blocked:number;failed:number;cancelled:number;recovery_required:number;unchanged:number}
export interface ImportProgress {operation_id:string;stage:string;completed:number;issue_summary:string|null}
