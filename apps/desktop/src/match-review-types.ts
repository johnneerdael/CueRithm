import type {CatalogFacts} from "./catalog-facts-types";
import type {MetadataArtworkAsset,MetadataEditBatch} from "./editor-types";
export type ProviderFailureKind="authentication"|"unavailable"|"rate_limited"|"timeout"|"transport"|"parse"|"invalid_response"|"unknown";
export interface MatchReviewProviderAccess{id:string;available:boolean;requires_solver:boolean;reason:string|null}
export interface MatchReviewProviderWarning{provider:string;kind:ProviderFailureKind;message:string}
export type MatchReviewFieldAction="keep"|"replace"|"append";
export interface MatchReviewSource{title:string|null;artists:string[];album:string|null;version:string|null;duration_seconds:number;has_artwork:boolean}
export interface MatchReviewCandidate{facts?:CatalogFacts|null;id:string;provider:string;track_id:string|null;release_id:string|null;title:string;version:string|null;artists:string[];album:string|null;isrc:string|null;duration_seconds:number|null;similarity:number;identity_conflicts:string[];source_url:string|null;artwork_url:string|null;selectable:boolean;reason:string|null}
export interface MatchReviewState{track_id:string;track_state:string;job_id:string|null;attempt:number;generation:string;source_sha256:string;settings_revision:string;status:string;source:MatchReviewSource;candidates:MatchReviewCandidate[];truncated:boolean;can_requery:boolean;reason:string|null;can_keep_existing_once:boolean;providers:MatchReviewProviderAccess[];warnings:MatchReviewProviderWarning[]}
export interface MatchReviewRequest{track_id:string;generation:string;action:"use_selected"|"keep_existing_once";candidate_id:string|null;fields:{field:string;action:MatchReviewFieldAction}[];acknowledge_conflicts:boolean;allow_id3_upgrade:boolean}
export interface MatchReviewFieldDiff{field:string;label:string;before:string[];proposed:string[];after:string[];action:MatchReviewFieldAction;allowed_actions:MatchReviewFieldAction[];reason:string|null}
export interface MatchReviewPreview{id:string;revision:string;track_id:string;job_id:string|null;attempt:number;generation:string;action:MatchReviewRequest["action"];candidate:MatchReviewCandidate|null;fields:MatchReviewFieldDiff[];artwork:{existing:boolean;candidate_url:string|null;action:MatchReviewFieldAction;reason:string|null};requires_id3_upgrade:boolean;can_apply:boolean;blockers:string[]}
export interface MatchReviewApplyResult{state:string;job_id:string|null;metadata_batch:MetadataEditBatch|null}
export interface MatchReviewBridge{
 openCatalogSource(provider:string,url:string):Promise<void>;
 trackMatchReview(trackId:string):Promise<MatchReviewState>;
 previewMatchReview(operationId:string,request:MatchReviewRequest):Promise<MatchReviewPreview>;
 applyMatchReview(operationId:string,id:string,revision:string):Promise<MatchReviewApplyResult>;
 requeryMatches(operationId:string,trackId:string,generation:string):Promise<MatchReviewState>;
 matchReviewArtwork(operationId:string,previewId:string,revision:string):Promise<MetadataArtworkAsset>;
 onMatchReviewChange(handler:()=>void):Promise<()=>void>;
}
