import type {MeasuredMetadata,ValueEdit} from "./editor-types";
export type CurationGroupMode="single"|"multiple";
export interface CurationValue{id:string;name:string;position:number}
export interface CurationGroup{id:string;name:string;mode:CurationGroupMode;position:number;values:CurationValue[]}
export type CurationContainer="id3"|"vorbis"|"mp4"|"asf";
export type CurationTextField="genre"|"style"|"grouping"|"label"|"version"|"composer"|"lyricist";
export type CurationEncoding="values"|"json";
export type CurationMappingSource={kind:"group";group_id:string}|{kind:"rating"}|{kind:"notes"};
export type CurationMappingTarget={kind:"text";field:CurationTextField;encoding:CurationEncoding}|{kind:"comment";language:string;description:string;encoding:CurationEncoding}|{kind:"rating";owner:string};
export interface CurationMapping{id:string;source:CurationMappingSource;target:CurationMappingTarget;containers:CurationContainer[];enabled:boolean;allow_id3_upgrade:boolean}
export interface CurationCatalog{revision:number;groups:CurationGroup[];mappings:CurationMapping[];capabilities:{native_available:boolean;containers:CurationContainer[];text_fields:CurationTextField[];native_dj_reason:string|null}}
export type CurationTaxonomyChange={kind:"create_group";name:string;mode:CurationGroupMode}|{kind:"rename_group";id:string;name:string}|{kind:"set_group_mode";id:string;mode:CurationGroupMode}|{kind:"delete_group";id:string;replacement_id:string|null}|{kind:"create_value";group_id:string;name:string}|{kind:"rename_value";id:string;name:string}|{kind:"delete_value";id:string;replacement_id:string|null}|{kind:"put_mapping";mapping:CurationMapping}|{kind:"delete_mapping";id:string};
export interface CurationTaxonomyRequest{request_id:string;expected_revision:number;change:CurationTaxonomyChange}
export interface CurationTaxonomyReceipt{revision:number;created_id:string|null;affected_tracks:number;reexport_required:number}
export interface CurationValues{categories:Record<string,string[]>;user_rating:number|null;notes:string}
export type CurationGroupEdit={kind:"set"|"add"|"remove";group_id:string;value_ids:string[]}|{kind:"clear";group_id:string};
export interface CurationPatch{groups:CurationGroupEdit[];user_rating:ValueEdit<number>;notes:ValueEdit<string>}
export const emptyCurationPatch=():CurationPatch=>({groups:[],user_rating:{kind:"keep"},notes:{kind:"keep"}});
export interface CurationExpectedRevision{track_id:string;revision:number}
export interface CurationUpdateRequest{request_id:string;taxonomy_revision:number;tracks:CurationExpectedRevision[];patch:CurationPatch}
export interface CurationReceipt{request_id:string;accepted:{track_id:string;revision:number}[];library_revision:number}
export interface CurationTrackState{track_id:string;title:string;desired_revision:number;acknowledged_revision:number;desired:CurationValues;committed:CurationValues;state:string;error:string|null;active_publication_id:string|null;native_stale:boolean;can_retry:boolean;can_discard:boolean;measured:MeasuredMetadata}
export interface CurationSnapshot{taxonomy_revision:number;library_revision:number;tracks:CurationTrackState[];mixed:string[]}
export type CurationPending=Omit<CurationTrackState,"desired"|"committed"|"active_publication_id"|"measured">;
export interface CurationPage<T>{total:number;offset:number;limit:number;rows:T[]}
export interface CurationPublicationSummary{id:string;track_id:string;title:string;revision:number;state:string;native_required:boolean;created:number;error:string|null;can_undo:boolean;can_recover:boolean}
export interface CurationPublication{summary:CurationPublicationSummary;before:CurationValues;after:CurationValues;taxonomy_revision:number;child_batch_id:string|null;undo_of:string|null}
export interface CurationReexportRequest{request_id:string;taxonomy_revision:number;tracks:CurationExpectedRevision[]}
export interface CurationFilter{all_tags:string[];any_tags:string[];exclude_tags:string[];empty_groups:string[];user_rating_min:number|null;user_rating_max:number|null;notes:string}
export const emptyCurationFilter=():CurationFilter=>({all_tags:[],any_tags:[],exclude_tags:[],empty_groups:[],user_rating_min:null,user_rating_max:null,notes:""});
export interface CurationEvent{track_id:string|null;revision:number;state:string}
export interface CurationBridge{
 curationCatalog():Promise<CurationCatalog>;
 editCurationTaxonomy(request:CurationTaxonomyRequest):Promise<CurationTaxonomyReceipt>;
 curationSnapshot(trackIds:string[]):Promise<CurationSnapshot>;
 updateCuration(request:CurationUpdateRequest):Promise<CurationReceipt>;
 reexportCuration(request:CurationReexportRequest):Promise<CurationReceipt>;
 curationPendingPage(offset:number,limit:number):Promise<CurationPage<CurationPending>>;
 curationHistoryPage(offset:number,limit:number):Promise<CurationPage<CurationPublicationSummary>>;
 curationPublication(id:string):Promise<CurationPublication>;
 retryCuration(operationId:string,trackId:string,expectedRevision:number):Promise<CurationTrackState>;
 discardCuration(trackId:string,expectedRevision:number):Promise<CurationTrackState>;
 undoCuration(operationId:string,publicationId:string):Promise<CurationPublication>;
 onCurationChange(handler:(event:CurationEvent)=>void):Promise<()=>void>;
}
