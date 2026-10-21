import {useCallback,useEffect,useRef,useState} from "react";
import type {Bridge} from "../types";
import type {MetadataDraft,MetadataEditBatch,MetadataEditHistoryPage,MetadataEditorPatch} from "../editor-types";
import {errorMessage} from "../api";
import {inspectorOperation} from "../inspector-operations";
import InspectorFrame from "./InspectorFrame";
import MetadataEditResult from "./MetadataEditResult";
import CurationHistory from "./CurationHistory";
export function hasDraftChanges(patch:MetadataEditorPatch){return !!patch.fields.allow_id3_upgrade||!!Object.keys(patch.form_values??{}).length||patch.artwork.length>0||Object.values(patch.fields.text).some(value=>value?.kind!=="keep")||(["release_date","original_date","track_number","track_total","disc_number","disc_total","bpm"] as const).some(field=>patch.fields[field].kind!=="keep")||patch.fields.ratings.some(value=>value.value.kind!=="keep")||patch.fields.comments.some(value=>value.value.kind!=="keep");}
const PAGE_SIZE=25;
export default function MetadataPending({bridge,onOpenDraft,onOpenCuration,onClose,onChanged,initialTab="metadata"}:{bridge:Bridge;onOpenCuration?:(id:string)=>void;initialTab?:"metadata"|"curation";onOpenDraft:(draft:MetadataDraft)=>void;onClose:()=>void|Promise<void>;onChanged:()=>void}){
 const [tab,setTab]=useState(initialTab);
 const [drafts,setDrafts]=useState<MetadataDraft[]>([]),[page,setPage]=useState<MetadataEditHistoryPage>({total:0,offset:0,limit:PAGE_SIZE,batches:[]}),[detail,setDetail]=useState<MetadataEditBatch|null>(null),[busy,setBusy]=useState(false),[loading,setLoading]=useState(false),[error,setError]=useState<string|null>(null);
 const alive=useRef(true),generation=useRef(0),detailGeneration=useRef(0),flight=useRef(false),offset=useRef(0),detailId=useRef<string|null>(null);
 const refresh=useCallback(async(nextOffset=offset.current)=>{
  const request=++generation.current;setLoading(true);
  try{
   const [nextDrafts,nextPage]=await Promise.all([bridge.metadataDrafts(),bridge.metadataEditHistoryPage(nextOffset,PAGE_SIZE)]);
   if(!alive.current||request!==generation.current)return;
   if(nextPage.total&&nextPage.offset>=nextPage.total){offset.current=Math.floor((nextPage.total-1)/PAGE_SIZE)*PAGE_SIZE;const valid=await bridge.metadataEditHistoryPage(offset.current,PAGE_SIZE);if(!alive.current||request!==generation.current)return;setPage(valid);}else{offset.current=nextPage.offset;setPage(nextPage);}
   setDrafts(nextDrafts.filter(draft=>!['saved','discarded'].includes(draft.state)&&(hasDraftChanges(draft.patch)||!!draft.error)));
  }finally{if(alive.current&&request===generation.current)setLoading(false);}
 },[bridge]);
 const readDetail=useCallback(async(id:string)=>{const request=++detailGeneration.current;detailId.current=id;const next=await bridge.metadataEditBatch(id);if(alive.current&&request===detailGeneration.current&&detailId.current===id)setDetail(next);},[bridge]);
 useEffect(()=>{alive.current=true;void refresh().catch(e=>{if(alive.current)setError(errorMessage(e));});const listener=bridge.onMetadataEditChange(()=>{void refresh().catch(e=>{if(alive.current)setError(errorMessage(e));});if(detailId.current)void readDetail(detailId.current).catch(e=>{if(alive.current)setError(errorMessage(e));});});return()=>{alive.current=false;generation.current++;detailGeneration.current++;void listener.then(dispose=>dispose());};},[bridge,refresh,readDetail]);
 const run=async(action:()=>Promise<unknown>)=>{
  if(flight.current)return;flight.current=true;setBusy(true);setError(null);let failure:unknown;
  try{const result=await action();if(result&&typeof result==='object'&&'rows' in result){const batch=result as MetadataEditBatch;detailId.current=batch.id;setDetail(batch);}}catch(e){failure=e;}
  finally{onChanged();try{await refresh();if(detailId.current)await readDetail(detailId.current);}catch(e){failure??=e;}flight.current=false;if(alive.current){setBusy(false);if(failure)setError(errorMessage(failure));}}
 };
 const navigate=(next:number)=>{detailGeneration.current++;detailId.current=null;setDetail(null);setError(null);void refresh(next).catch(e=>{if(alive.current)setError(errorMessage(e));});};
 return <InspectorFrame title="Drafts and history" description="Unfinished edits stay here across navigation and restart." onClose={onClose} actions={<button className="secondary" disabled={busy||loading} onClick={()=>void refresh().catch(e=>setError(errorMessage(e)))}>Refresh</button>}>
  <nav className="curation-pending-tabs" aria-label="Pending change types"><button className="secondary" aria-pressed={tab==="metadata"} onClick={()=>setTab("metadata")}>Metadata edits</button><button className="secondary" aria-pressed={tab==="curation"} onClick={()=>setTab("curation")}>Curate</button></nav>{tab==="curation"?<CurationHistory bridge={bridge} onOpenTrack={id=>onOpenCuration?.(id)} onChanged={onChanged}/>:<>
  {error?<p role="alert" className="inspector-error">{error}</p>:null}{busy||loading?<p role="status">Updating metadata history…</p>:null}
  <section className="metadata-section"><h3>Pending drafts <small>{drafts.length}</small></h3>{!drafts.length?<p className="setting-note">No unfinished metadata edits.</p>:drafts.map(draft=><article className="metadata-pending-row" key={draft.id}><div><strong>{draft.track_ids.length} {draft.track_ids.length===1?'track':'tracks'}</strong><small>Revision {draft.revision} · {draft.state.replaceAll('_',' ')}</small>{draft.error?<p className="attention">{draft.error}</p>:null}</div><div><button className="secondary" disabled={busy} onClick={()=>onOpenDraft(draft)}>Open draft</button>{!['queued','saving'].includes(draft.state)?<button className="back-link" disabled={busy} onClick={()=>void run(()=>bridge.discardMetadataDraft(draft.id,draft.revision))}>Discard draft</button>:null}</div></article>)}</section>
  <section className="metadata-section"><h3>Publication history</h3>{!page.total?<p className="setting-note">Saved changes and recoverable operations appear here.</p>:<><nav className="artwork-actions" aria-label="Publication history pages"><button className="secondary" disabled={busy||loading||page.offset===0} onClick={()=>navigate(Math.max(0,page.offset-PAGE_SIZE))}>Newer changes</button><span>{page.offset+1}–{page.offset+page.batches.length} of {page.total}</span><button className="secondary" disabled={busy||loading||page.offset+page.batches.length>=page.total} onClick={()=>navigate(page.offset+PAGE_SIZE)}>Older changes</button></nav>{page.batches.map(batch=><article className="metadata-history-row" key={batch.id}><div className="metadata-pending-row"><div><strong>{batch.state.replaceAll('_',' ')}</strong><small>{new Date(batch.created).toLocaleString()} · Revision {batch.draft_revision}</small><small>{Object.entries(batch.counts).map(([state,count])=>count+' '+state.replaceAll('_',' ')).join(' · ')||'No track outcomes'}</small>{batch.error?<p className="attention">{batch.error}</p>:null}</div><button className="secondary" disabled={busy||loading} aria-expanded={detail?.id===batch.id} onClick={()=>{setError(null);void readDetail(batch.id).catch(e=>{if(alive.current)setError(errorMessage(e));});}}>Details</button></div>{detail?.id===batch.id?<MetadataEditResult result={detail}/>:null}<div className="artwork-actions">{batch.can_recover?<button className="primary" disabled={busy||loading} onClick={()=>void run(()=>inspectorOperation(bridge,id=>bridge.recoverMetadataEdits(id,batch.id)))}>Retry recovery</button>:null}{batch.can_undo?<button className="secondary" disabled={busy||loading} onClick={()=>void run(()=>inspectorOperation(bridge,id=>bridge.undoMetadataEdits(id,batch.id)))}>Undo saved changes</button>:null}</div></article>)}</>}</section>
 </>}
 </InspectorFrame>;
}
