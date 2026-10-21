import {vi} from "vitest";
import {fakeBridge,track} from "./bridge";
import {emptyMetadataPatch,type MetadataDraft,type MetadataEditPreview,type MetadataEditBatch,type MetadataValues} from "../editor-types";
export const nativeValues=(title:string):MetadataValues=>({text:{title:[title],artist:["Artist"],genre:["House"]},release_date:null,original_date:null,track_number:null,track_total:null,disc_number:null,disc_total:null,bpm:null,ratings:[],comments:[],artwork:[],capabilities:{}});
export function editorBridge(ids=["a"]){
 const bridge=fakeBridge(ids.map(track)),drafts=new Map<string,MetadataDraft>(),previews=new Map<string,MetadataEditPreview>(),history:MetadataEditBatch[]=[];
 let counter=0;
 bridge.metadataDrafts=vi.fn(async()=>[...drafts.values()].map(value=>structuredClone(value)));
 bridge.metadataDraft=vi.fn(async id=>structuredClone(drafts.get(id)!));
 bridge.metadataEditHistoryPage=vi.fn(async(offset:number,limit:number)=>({total:history.length,offset,limit,batches:history.slice().sort((a,b)=>b.created-a.created).slice(offset,offset+limit).map(batch=>({id:batch.id,draft_id:batch.draft_id,draft_revision:batch.draft_revision,state:batch.state,created:batch.created,can_undo:batch.can_undo,can_recover:batch.can_recover,error:batch.error,counts:batch.rows.reduce<Record<string,number>>((counts,row)=>{counts[row.outcome]=(counts[row.outcome]??0)+1;return counts;},{})}))}));
 bridge.metadataEditBatch=vi.fn(async id=>{const batch=history.find(batch=>batch.id===id);if(!batch)throw Error("Unknown batch");return structuredClone(batch);});
 bridge.metadataEditorSnapshot=vi.fn(async(_op:string,trackIds:string[])=>({mixed:trackIds.length>1?["title"]:[],tracks:trackIds.map(id=>({track_id:id,title:"Track "+id,display_path:"/music/"+id+".flac",state:"ready",source_sha256:"hash-"+id,path_revision:1,metadata_revision:1,native:nativeValues("Track "+id),measured:{key:"8A",bpm:128,energy:6,duration_seconds:240}}))}));
 bridge.createMetadataDraft=vi.fn(async(_op:string,trackIds:string[])=>{const draft:MetadataDraft={id:"draft-"+(++counter),revision:1,track_ids:trackIds,patch:emptyMetadataPatch(),state:"editing",created:1,updated:1,error:null};drafts.set(draft.id,draft);return structuredClone(draft);});
 bridge.updateMetadataDraft=vi.fn(async(id,revision,patch)=>{const old=drafts.get(id)!;if(old.revision!==revision)throw Error("Draft revision changed");const next={...old,revision:revision+1,patch:structuredClone(patch)};drafts.set(id,next);return structuredClone(next);});
 bridge.previewMetadataEdits=vi.fn(async(_op,id,revision)=>{const draft=drafts.get(id)!;if(draft.revision!==revision)throw Error("Draft changed");const preview:MetadataEditPreview={id:"preview-"+(++counter),draft_id:id,draft_revision:revision,created:1,can_apply:true,rows:draft.track_ids.map(id=>({track_id:id,title:"Track "+id,before:nativeValues("Track "+id),after:nativeValues("Updated "+id),outcome:"ready",fields:[{field:"title",status:"ready",reason:null}],error:null}))};previews.set(preview.id,preview);return preview;});
 bridge.applyMetadataEdits=vi.fn(async(_op,id)=>{const preview=previews.get(id)!,draft=drafts.get(preview.draft_id)!;if(draft.revision!==preview.draft_revision)throw Error("Preview is stale");const result:MetadataEditBatch={...preview,id:"batch-"+(++counter),state:"saved",rows:preview.rows.map(row=>({...row,outcome:"written"})),can_undo:true,can_recover:false,error:null};drafts.set(draft.id,{...draft,state:"saved"});history.push(result);return result;});
 return {bridge,drafts,previews,history};
}
