import {useEffect,useRef,useState} from "react";
import type {CurationBridge,CurationPatch,CurationSnapshot,CurationTrackState,CurationUpdateRequest,CurationValues} from "./curation-types";
import {emptyCurationPatch} from "./curation-types";
import {errorMessage} from "./api";
export function applyCurationPatch(before:CurationValues,patch:CurationPatch):CurationValues{
 const value=structuredClone(before);
 for(const edit of patch.groups){const old=value.categories[edit.group_id]??[];value.categories[edit.group_id]=edit.kind==="clear"?[]:edit.kind==="set"?[...edit.value_ids]:edit.kind==="add"?[...new Set([...old,...edit.value_ids])]:old.filter(id=>!edit.value_ids.includes(id));}
 if(patch.user_rating.kind!=="keep")value.user_rating=patch.user_rating.kind==="clear"?null:patch.user_rating.value;
 if(patch.notes.kind!=="keep")value.notes=patch.notes.kind==="clear"?"":patch.notes.value;
 return value;
}
interface Operation{sequence:number;patch:CurationPatch;taxonomy:number;request?:CurationUpdateRequest}
export function useCurationDraft(bridge:CurationBridge,initial:CurationSnapshot){
 const [tracks,setTracks]=useState(initial.tracks),[error,setError]=useState<string|null>(null),[localPending,setLocalPending]=useState(false),[discarded,setDiscarded]=useState(false);
 const alive=useRef(true),base=useRef(structuredClone(initial)),sequence=useRef(0),durable=useRef(0),ops=useRef<Operation[]>([]),queue=useRef<Promise<void>>(Promise.resolve()),refreshRequest=useRef(0),abandoning=useRef(false);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;refreshRequest.current++;};},[]);
 async function refresh(){
  const request=++refreshRequest.current,last=queue.current;await last.catch(()=>{});
  if(!alive.current||request!==refreshRequest.current||last!==queue.current||ops.current.length)return;
  const observedSequence=sequence.current,next=await bridge.curationSnapshot(base.current.tracks.map(t=>t.track_id));
  if(!alive.current||request!==refreshRequest.current||observedSequence!==sequence.current||ops.current.length)return;
  if(next.tracks.some(row=>row.desired_revision<(base.current.tracks.find(old=>old.track_id===row.track_id)?.desired_revision??0)))return;
  base.current=structuredClone(next);setTracks(next.tracks);
 }
 function pump(){
  queue.current=queue.current.catch(()=>{}).then(async()=>{
   while(ops.current.length){
    const op=ops.current[0];op.request??={request_id:crypto.randomUUID(),taxonomy_revision:op.taxonomy,tracks:base.current.tracks.map(row=>({track_id:row.track_id,revision:row.desired_revision})),patch:op.patch};
    const receipt=await bridge.updateCuration(op.request);
    if(receipt.request_id!==op.request.request_id||receipt.accepted.length!==base.current.tracks.length||new Set(receipt.accepted.map(r=>r.track_id)).size!==base.current.tracks.length||base.current.tracks.some(row=>!receipt.accepted.some(r=>r.track_id===row.track_id&&Number.isSafeInteger(r.revision)&&(r.revision>row.desired_revision||r.revision===row.desired_revision&&JSON.stringify(applyCurationPatch(row.desired,op.patch))===JSON.stringify(row.desired)))))throw Error("Curate receipt did not acknowledge the exact draft revision.");
    base.current={...base.current,library_revision:receipt.library_revision,tracks:base.current.tracks.map(row=>({...row,desired_revision:receipt.accepted.find(r=>r.track_id===row.track_id)!.revision,desired:applyCurationPatch(row.desired,op.patch),state:"pending"}))};
    durable.current=op.sequence;ops.current.shift();
    if(alive.current)setTracks(current=>current.map(row=>({...row,desired_revision:base.current.tracks.find(r=>r.track_id===row.track_id)!.desired_revision,state:"pending"})));
   }
   if(alive.current){setLocalPending(durable.current<sequence.current);setError(null);void refresh().catch(e=>{if(alive.current)setError(errorMessage(e));});}
  }).catch(e=>{if(alive.current){setError(errorMessage(e));setLocalPending(false);}throw e;});
  void queue.current.catch(()=>{});
 }
 function enqueue(patch:CurationPatch,id:number){const last=ops.current.at(-1),notesOnly=(value:CurationPatch)=>!value.groups.length&&value.user_rating.kind==="keep"&&value.notes.kind!=="keep";if(last&&!last.request&&notesOnly(last.patch)&&notesOnly(patch)){last.patch=structuredClone(patch);last.sequence=id;}else ops.current.push({sequence:id,patch:structuredClone(patch),taxonomy:base.current.taxonomy_revision});pump();}

 function optimistic(patch:CurationPatch){setTracks(current=>current.map(row=>({...row,desired:applyCurationPatch(row.desired,patch)})));setLocalPending(true);setError(null);setDiscarded(false);}
 function edit(patch:CurationPatch){if(abandoning.current)return;const id=++sequence.current;optimistic(patch);enqueue(patch,id);}
 function editNotes(value:string){edit({...emptyCurationPatch(),notes:{kind:"set",value}});}
 async function flush(){for(;;){const current=queue.current;await current;if(current!==queue.current)continue;if(durable.current<sequence.current)throw Error("The latest Curate edit has not been saved locally.");return;}}
 async function retry(){setError(null);setLocalPending(true);pump();await flush();}
 async function discardUnsaved(){abandoning.current=true;try{for(;;){const current=queue.current;await current.catch(()=>{});if(current===queue.current)break;}ops.current=[];durable.current=++sequence.current;queue.current=Promise.resolve();if(alive.current){setTracks(structuredClone(base.current.tracks));setError(null);setLocalPending(false);setDiscarded(true);void refresh().catch(e=>{if(alive.current)setError(errorMessage(e));});}}finally{abandoning.current=false;}}
 return {tracks,error,localPending,discarded,edit,editNotes,flush,retry,discardUnsaved,refresh,dirty:durable.current<sequence.current};
}
