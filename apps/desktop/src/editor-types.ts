export type MetadataTextField="title"|"artist"|"album"|"album_artist"|"genre"|"style"|"label"|"version"|"remixer"|"producer"|"composer"|"lyricist"|"grouping"|"comment"|"catalog_number"|"isrc"|"key";
export type TextEdit={kind:"keep"}|{kind:"set";values:string[]}|{kind:"clear"}|{kind:"append";values:string[]};
export type ValueEdit<T>={kind:"keep"}|{kind:"set";value:T}|{kind:"clear"};
export interface MetadataDate{year:number;month:number|null;day:number|null}
export interface RatingIdentity{owner:string}
export interface NativeRating{identity:RatingIdentity;raw:number;counter:number;stars:number|null}
export interface CommentIdentity{language:string;description:string;sha256:string}
export interface NativeComment{identity:CommentIdentity;values:string[]}
export interface ArtworkIdentity{kind:number|null;description:string|null;sha256:string;ordinal:number}
export interface NativeArtwork{identity:ArtworkIdentity;mime:string;bytes:number}
export interface MetadataValues{
  text:Partial<Record<MetadataTextField,string[]>>;
  release_date:MetadataDate|null;original_date:MetadataDate|null;
  track_number:number|null;track_total:number|null;disc_number:number|null;disc_total:number|null;bpm:number|null;
  ratings:NativeRating[];comments:NativeComment[];artwork:NativeArtwork[];capabilities:Record<string,string>;
}
export interface MetadataPatch{
  allow_id3_upgrade:boolean;
  text:Partial<Record<MetadataTextField,TextEdit>>;
  release_date:ValueEdit<MetadataDate>;original_date:ValueEdit<MetadataDate>;
  track_number:ValueEdit<number>;track_total:ValueEdit<number>;disc_number:ValueEdit<number>;disc_total:ValueEdit<number>;bpm:ValueEdit<number>;
  ratings:{identity:RatingIdentity;value:ValueEdit<number>}[];
  comments:{identity:CommentIdentity;value:TextEdit}[];
  /** Native image bytes stay behind the asset API. */
  artwork:never[];
}
export interface MetadataArtworkChoice{target:ArtworkIdentity|null;asset_id:string|null;kind:number|null;description:string|null}
export interface MetadataEditorPatch{fields:MetadataPatch;artwork:MetadataArtworkChoice[];form_values:Record<string,string>}
export interface MetadataArtworkAsset{id:string;mime:string;sha256:string;bytes:number}
export interface MeasuredMetadata{key:string|null;bpm:number|null;energy:number|null;duration_seconds:number|null}
export interface MetadataEditTrack{track_id:string;title:string;display_path:string;state:string;source_sha256:string;path_revision:number;metadata_revision:number;native:MetadataValues;measured:MeasuredMetadata}
export interface MetadataEditorSnapshot{tracks:MetadataEditTrack[];mixed:string[]}
export interface MetadataDraft{id:string;revision:number;track_ids:string[];patch:MetadataEditorPatch;state:string;created:number;updated:number;error:string|null}
export interface MetadataFieldOutcome{field:string;status:string;reason:string|null}
export interface MetadataEditRow{track_id:string;title:string;before:MetadataValues;after:MetadataValues;outcome:string;fields:MetadataFieldOutcome[];error:string|null}
export interface MetadataEditPreview{id:string;draft_id:string;draft_revision:number;created:number;rows:MetadataEditRow[];can_apply:boolean}
export interface MetadataEditBatch{id:string;draft_id:string;draft_revision:number;state:string;rows:MetadataEditRow[];created:number;can_undo:boolean;can_recover:boolean;error:string|null}
export interface MetadataEditSummary{id:string;draft_id:string;draft_revision:number;state:string;created:number;can_undo:boolean;can_recover:boolean;error:string|null;counts:Record<string,number>}
export interface MetadataEditHistoryPage{total:number;offset:number;limit:number;batches:MetadataEditSummary[]}
export interface MetadataEditorBridge{
  onMetadataEditChange(handler:()=>void):Promise<()=>void>;
  metadataEditorSnapshot(operationId:string,trackIds:string[]):Promise<MetadataEditorSnapshot>;
  createMetadataDraft(operationId:string,trackIds:string[]):Promise<MetadataDraft>;
  metadataDraft(id:string):Promise<MetadataDraft>;
  metadataDrafts():Promise<MetadataDraft[]>;
  updateMetadataDraft(id:string,expectedRevision:number,patch:MetadataEditorPatch):Promise<MetadataDraft>;
  previewMetadataEdits(operationId:string,id:string,revision:number):Promise<MetadataEditPreview>;
  applyMetadataEdits(operationId:string,previewId:string):Promise<MetadataEditBatch>;
  recoverMetadataEdits(operationId:string,batchId:string):Promise<MetadataEditBatch>;
  undoMetadataEdits(operationId:string,batchId:string):Promise<MetadataEditBatch>;
  discardMetadataDraft(id:string,revision:number):Promise<void>;
  metadataEditHistoryPage(offset:number,limit:number):Promise<MetadataEditHistoryPage>;
  metadataEditBatch(id:string):Promise<MetadataEditBatch>;
  chooseMetadataArtwork():Promise<MetadataArtworkAsset|null>;
  extractMetadataArtwork(operationId:string,trackId:string,sourceSha256:string,identity:ArtworkIdentity):Promise<MetadataArtworkAsset>;
  metadataArtworkAsset(assetId:string):Promise<string>;
  cancelMetadataOperation(operationId:string):Promise<boolean>;
}
export function emptyMetadataPatch():MetadataEditorPatch{return {fields:{allow_id3_upgrade:false,text:{},release_date:{kind:"keep"},original_date:{kind:"keep"},track_number:{kind:"keep"},track_total:{kind:"keep"},disc_number:{kind:"keep"},disc_total:{kind:"keep"},bpm:{kind:"keep"},ratings:[],comments:[],artwork:[]},artwork:[],form_values:{}};}
