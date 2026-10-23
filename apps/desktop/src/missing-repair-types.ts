import type {MaintenanceFileState,MaintenanceStamp} from "./maintenance-types";

export type MissingRepairKind="remove"|"relink";
export type MissingRowOutcome="ready"|"blocked"|"not_found"|"ambiguous"|"already_tracked";
export interface MissingRow {track_id:string;title:string;artist:string;archived:boolean;display_path:string;file_state:MaintenanceFileState;outcome:MissingRowOutcome;reason:string|null;new_path:string|null;playlists:number;sync_links:number;rekordbox_links:number}
export interface MissingImpact {playlists:{id:string;name:string;tracks:number}[];playlist_memberships:number;sync_links:number;rekordbox_links:number;archived:number;unreachable:number}
export interface MissingPreview {id:string;kind:MissingRepairKind;scan_id:string;created:number;rows:MissingRow[];ready:number;impact:MissingImpact;locations:string[];files_checked:number;undo_limit?:number}
export interface RepairedTrack {id:string;title:string;artist:string;archived:boolean;display_path:string;new_path:string|null}
export interface MissingRepairEntry {id:string;kind:MissingRepairKind;preview_id:string|null;created:number;undo_of:string|null;track_count:number;tracks:RepairedTrack[];skipped:{id:string;title:string;reason:string}[];notes?:string[];playlist_ids:string[];playlist_memberships:number;sync_links:number;rekordbox_links:number;stamp:MaintenanceStamp}
export interface MissingRepairHistory {entry:MissingRepairEntry;undone_by:string|null;undo_discarded?:boolean;undo_limit?:number}
