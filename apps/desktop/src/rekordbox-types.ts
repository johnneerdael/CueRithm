export interface RekordboxInstallation {version:string;major:number}
export interface NativeTarget {id:string;identity:{database_id:string;device_id:string|null;database_version:string|null;local_update_count:number|null};schema_fingerprint:string;revision:number;records:number;tracks:number;issues:{table:string;row_id:string;field:string;code:string}[]}
export interface RekordboxPreflight {platform_supported:boolean;installations:RekordboxInstallation[];database_path:string|null;database_state:string|null;key_ready:boolean;key_error:string|null;schema_fingerprint:string|null;write_supported:boolean;write_profiles:string[];targets:NativeTarget[]}
export interface RekordboxPlaylist {id:string;parent_id:string|null;name:string;is_folder:boolean;tracks:number}
export type ImportScope={kind:"all"}|{kind:"playlist";id:string};
export interface NativeImportReport {target_id:string;revision:number;changed:boolean;tracks_created:number;jobs_created:number;statuses:Record<string,number>}
export type FieldDecision="unchanged"|"converged"|"push_local"|"pull_remote"|"conflict"|"needs_owner"|"empty_local";
export interface FieldDiff {field:string;decision:FieldDecision;local:unknown;remote:unknown;baseline_local:unknown;baseline_remote:unknown;owner:"local"|"remote"|"shared"|"unowned";acknowledged:boolean}
export interface TrackPreview {content_id:string;local_track_id:string|null;status:string;expected_fingerprint:string|null;blocked:string|null;fields:FieldDiff[]}
export interface NativeSyncPreview {target_id:string;revision:number;tracks:TrackPreview[];summary:Record<string,number>;writes_enabled:boolean;write_blocker:string}
export interface PlannedTrack {content_id:string;local_track_id:string|null;fields:string[]}
export interface SkippedField {content_id:string;field:string;decision:FieldDecision}
export interface SyncPlanSummary {tracks:PlannedTrack[];skipped:SkippedField[];blocked:[string,string][];pull_available:number}
export type PlaylistAction="create"|"update"|"unchanged"|"remove"|"skip_existing"|"skip_changed";
export interface PlaylistPlanItem {path:string[];folder:boolean;action:PlaylistAction;tracks:number;unlinked:number}
export interface PlaylistPlanSummary {items:PlaylistPlanItem[]}
export interface NativeSyncPrepared {target_id:string;preview:NativeSyncPreview;plan:SyncPlanSummary;playlists:PlaylistPlanSummary;snapshot_usn:number}
export interface NativeSyncRun {id:string;created:number;target_id:string;state:"committed"|"nothing_to_write";usn_before:number|null;usn_after:number|null;tracks:PlannedTrack[];playlists:PlaylistPlanSummary;skipped:number;backup:string|null;journal:string|null}
export interface NativeSyncConflict {subject:string;reason:string}
export interface ApplyOutcome {run:NativeSyncRun|null;conflicts:NativeSyncConflict[]}
