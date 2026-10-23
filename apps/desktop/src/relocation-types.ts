export interface RelocationRequest {mode:"selection"|"root";track_ids:string[];search_root:string|null;replacement_file:string|null;source_root:string|null;destination_root:string|null;extension:string|null;all_tracks:boolean;include_unavailable:boolean}
export interface RelocationCandidate {id:string;path:string;evidence:string;preselected:boolean;replacement:boolean;occupied_by:string|null;size_bytes:number;group_track_ids:string[];proof?:{path:number[];root:number[];size:number;mtime:string;hash:string;identity?:[number,number]|null}}
export interface RelocationRow {track_id:string;old_path:string;availability:string;state:string;candidates:RelocationCandidate[];decision:string}
export interface RelocationPlan {id:string;created:number;revision:number;rows:RelocationRow[];skipped:number;incomplete:boolean;unsearched:string[];unsearched_count?:number;paused:boolean}
export interface RelocationChoice {track_id:string;candidate_id:string;survivor:"skip"|"source"|"existing";survivor_track_id?:string|null}
export interface RelocationResult {operation_id:string;applied:number;no_op:number;skipped:number;backup_path:string|null;warnings:string[]}
