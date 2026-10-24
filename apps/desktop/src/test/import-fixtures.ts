import {vi} from "vitest";
import {importApi, type ImportBridge} from "../import-api";
import {defaultFields, type ImportInspection, type SourceProfile, type TrackObservation, type ImportPreview,type ImportPageQuery,type ImportPreviewRequest} from "../import-types";
export const importProfile:SourceProfile={provider:"mixxx",profile_id:"invented-mixxx-schema40",schema_fingerprint:"a".repeat(64),origin:{os:"unverified"},fields:Object.fromEntries(defaultFields.map(f=>[f,{read:true,write:false,lossless:true,evidence:"d2",reason:null}])),issues:[]};
export const importTrack:TrackObservation={external_id:"native-101",locations:{availability:"missing",original:"/example/missing.flac",resolved:null},metadata:{title:{presence:"value",value:"Imported title"},artist:{presence:"value",value:"Invented artist"}},cached_tags:{},key:{raw:"Am",camelot:"8A"},bpm:{value:120},energy:{presence:"absent"},grid:null,cues:[],extensions:{},issues:[]};
export const importInspection:ImportInspection={inspection_id:"inspection-one",source_id:"source-one",profile:importProfile,header:{provider:"mixxx",library_instance_id:"invented-library",product_version:"40",completeness:"whole_library"},counts:{tracks:1,collections:1,memberships:2,issues:0}};
export const importPreview:ImportPreview={preview_id:"server-held-preview",tracks_created:1,fields_changed:3,baselines_changed:3,collections_changed:1,conflicts:0,blocked:0,processing_requested:false};
export function importFixtureApi(overrides:Partial<ImportBridge>={}) {
 return {...importApi,
  discoverImportSources:vi.fn(async()=>[]),
  chooseImportSource:vi.fn(async()=>({selection_id:"native-choice",provider:"mixxx" as const,display_location:"/example/mixxx",artifacts:["mixxxdb.sqlite"],artifact_count:1})),
  inspectImport:vi.fn(async()=>structuredClone(importInspection)),
  importPage:vi.fn(async(_id:string,query:ImportPageQuery)=>({records:query.kind==="collection"?[{record_type:"collection" as const,record:{external_id:"playlist",parent_id:null,kind:"playlist",name:"Repeated playlist",occurrences:[{external_occurrence_id:"one",track_external_id:"native-101",position:1},{external_occurrence_id:"two",track_external_id:"native-101",position:2}]}}]:[{record_type:"track" as const,record:structuredClone(importTrack)}],total:1,next_cursor:null})),
  previewImport:vi.fn(async(_operationId:string,_request:ImportPreviewRequest)=>({...importPreview})),
  importPreviewPage:vi.fn(async()=>({effects:[{external_id:"native-101",item_id:"item-one",kind:"track",fields:[{field:"metadata.title",source:{presence:"value",value:"Imported title"},local:null,proposed:{presence:"value",value:"Imported title"},conflict:false,blocked_reason:null}]}],total:1,next_cursor:null})),
  applyImport:vi.fn(async(_operationId:string,_previewId:string)=>({operation_id:"durable-operation",tracks_created:1,fields_changed:3,collections_changed:1,processing_requested:false})),
  discardImportPreview:vi.fn(async()=>{}),
  importOperationPage:vi.fn(async()=>({records:[],total:0,next_cursor:null})),
  cancel:vi.fn(async()=>true),
  ...overrides
 } satisfies ImportBridge;
}
