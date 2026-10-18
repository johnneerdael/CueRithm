export type PlaylistKind="playlist"|"folder"|"smartlist";
import type {SmartOperator} from "./smart-rules";
export interface SmartRule {id:string;field:string;operator:SmartOperator;value:RuleValue;or:boolean}
export interface SmartDefinition {match_all:boolean;rules:SmartRule[]}
export type RuleValue={kind:"none"}|{kind:"text";value:string}|{kind:"number";value:number}|{kind:"number_range";value:{start:number;end:number}}|{kind:"date";value:number}|{kind:"date_range";value:{start:number;end:number}}|{kind:"relative";value:{count:number;unit:"days"|"weeks"|"months"}}|{kind:"color";value:string|null}|{kind:"ids";value:string[]}|{kind:"playlist";value:{id:string;kind:PlaylistKind;path:string[]}};
export interface PlaylistNode {id:string;parent_id:string|null;kind:PlaylistKind;name:string;position:number;favorite_position:number|null;definition:SmartDefinition|null;deleted:boolean;deleted_by:string|null}
export interface PlaylistState {revision:number;nodes:PlaylistNode[]}
export interface SmartStatus {playlist_id:string;state:"pending"|"ready"|"stale"|"error";error:string|null;evaluated_at:number|null;next_refresh:number|null;last_good_revision:number|null}
export interface PlaylistWorkspace {tree:PlaylistState;statuses:SmartStatus[];latest_undo:string|null;service_error:string|null}
export interface PlaylistChange {edit_id:string|null;revision:number;created_ids:string[];affected_ids:string[]}
export type PlaylistEdit={type:"create";id?:string|null;parent_id:string|null;kind:PlaylistKind;name:string;definition:SmartDefinition|null}|{type:"rename";id:string;name:string}|{type:"move";id:string;parent_id:string|null;position:number}|{type:"set_definition";id:string;definition:SmartDefinition}|{type:"duplicate";id:string;parent_id:string|null;name:string|null}|{type:"remove";id:string}|{type:"restore";id:string;parent_id:string|null;position:number}|{type:"favorite";id:string;position:number|null}|{type:"add_members"|"remove_members"|"reorder_members";id:string;track_ids:string[]}|{type:"batch";edits:PlaylistEdit[]};
export interface SmartStamp {input_revision:number;playlist_revision:number;preference_revision:number}
export interface SmartCatalog {time_zone:string;catalog:{fields:Record<string,{label:string;kind:string;any_text:boolean;unavailable_reason:string|null}>;tags:string[];categories:Record<string,string[]>;playlists:Record<string,PlaylistKind>};tags:{id:string;category_id:string;name:string}[];categories:{id:string;name:string}[];playlists:PlaylistNode[];stamp:SmartStamp}
export interface SmartPreview {id:string;track_ids:string[];stamp:SmartStamp;created:number;expires:number|null}
export interface PlaylistPreferences {time_zone:string|null;key_mixing_mode:"harmonic"|"fuzzy"}
export interface PlaylistPreferenceSnapshot {revision:number;preferences:PlaylistPreferences}
export type PlaylistPrefix={type:"number";start:number;leading_zero:boolean;replace_existing:boolean}|{type:"text";text:string};
export type PlaylistToolRequest={type:"combine";source_ids:string[];parent_id:string|null;name:string;mode:"union"|"intersection"|"absent_from_all"}|{type:"sort_siblings";parent_id:string|null;ids:string[]|null;order:"name_ascending"|"name_descending"|"track_count_descending"}|{type:"prefix";ids:string[];prefix:PlaylistPrefix}|{type:"uppercase";ids:string[]}|{type:"reorder_members";id:string;track_ids:string[]}|{type:"snapshot";source_id:string;parent_id:string|null;name:string};
export interface PlaylistToolPreview {id:string;stamp:SmartStamp;created:number;expires:number|null;changes:{id:string;name:string;previous_name:string|null;kind:PlaylistKind;parent_id:string|null;position:number;track_count:number;definition:SmartDefinition|null;is_new:boolean}[]}
export type GenerationDimension={type:"field";field:string;include_unobserved:boolean}|{type:"tags";tag_ids:string[]|null}|{type:"category";category_id:string}|{type:"decades"|"bpm_ranges"|"bitrate_buckets"|"size_buckets"|"duration_buckets"};
export interface SmartGenerationRequest {target:{type:"folder";id:string}|{type:"new_folder";parent_id:string|null;name:string};dimensions:GenerationDimension[];combine:boolean}
export interface GenerationPreview {id:string;stamp:SmartStamp;created:number;expires:number|null;folder_id:string|null;new_nodes:number;items:{id:string;parent_id:string;name:string;definition:SmartDefinition;track_count:number;is_new:boolean}[]}
