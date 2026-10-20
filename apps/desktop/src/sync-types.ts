export type DjSoftware="virtual_dj"|"engine_dj"|"djay_pro"|"serato4"|"pioneer_usb";
export interface SyncTarget {software:DjSoftware;path:string;source_root:string|null}
export interface DiscoveredTarget {software:DjSoftware;path:string}
export type SyncTextField="title"|"artist"|"album"|"genre"|"label"|"comment"|"key";
export interface SyncOptions {
  engine_copy_audio:boolean;
  materialize_smartlists:boolean;
  key_notation:"original"|"musical"|"camelot"|"open_key";
  field_mappings:Partial<Record<SyncTextField,SyncTextField>>;
  usb_cue_destination:"hot"|"memory"|"both";
  usb_cdj_compatibility:"off"|"detect"|"auto_fix";
  usb_cleanup_unused:boolean;
  skip_beatgrids:boolean;
  nearest_color:boolean;
  beatgrid_lock:"default"|"lock"|"unlock";
  add_album_art:boolean;
  extra_cue_markers:boolean;
}
export interface DjSyncRequest {target:SyncTarget;scope:{type:"full"}|{type:"playlist";id:string}|{type:"changed"};options:SyncOptions}
export interface FileChange {target:string;before_sha256:string|null;after_sha256:string|null;bytes:number}
export interface DirectoryChange {target:string;before_manifest:unknown;after_exists:boolean}
export interface CaseRenameChange {from:string;to:string}
export interface SyncPreview {renames?:CaseRenameChange[];deleted_playlists:number;retained_external_playlists:number;id:string;target:SyncTarget;tracks:number;playlists:number;bytes_to_write:number;files:FileChange[];warnings:string[];directories:DirectoryChange[]}
export type JournalState="prepared"|"applying"|"applied"|"restoring"|"restored";
export interface FileJournal {renames?:CaseRenameChange[];id:string;state:JournalState;changes:FileChange[];directories:DirectoryChange[]}
export interface DjSyncHistoryEntry {needs_acknowledgement:boolean;needs_restore_acknowledgement:boolean;id:string;state:JournalState|null;target:SyncTarget|null;tracks:number;playlists:number;created:number|null;error:string|null}
export const syncTargets:{id:DjSoftware;name:string;location:string}[]=[
  {id:"virtual_dj",name:"VirtualDJ",location:"VirtualDJ library folder"},
  {id:"engine_dj",name:"Engine DJ",location:"Engine Library folder or target drive"},
  {id:"djay_pro",name:"djay Pro",location:"djay Media Library folder"},
  {id:"serato4",name:"Serato DJ Pro 4",location:"Serato Library folder"},
  {id:"pioneer_usb",name:"Pioneer USB",location:"Target USB drive"},
];
export const defaultSyncOptions:SyncOptions={materialize_smartlists:false,engine_copy_audio:false,key_notation:"original",field_mappings:{},usb_cue_destination:"hot",usb_cdj_compatibility:"off",usb_cleanup_unused:false,skip_beatgrids:false,nearest_color:true,beatgrid_lock:"default",add_album_art:false,extra_cue_markers:false};
