import {createPortal} from "react-dom";
import {forwardRef,useCallback,useEffect,useImperativeHandle,useRef,useState} from "react";
import {LoaderCircle,ScanLine,Save,Undo2} from "lucide-react";
import type {Bridge} from "../types";
import type {MetadataDraft,MetadataEditorSnapshot,MetadataEditPreview,MetadataEditBatch} from "../editor-types";
import {useMetadataDraft} from "../use-metadata-draft";
import {inspectorOperation} from "../inspector-operations";
import {errorMessage} from "../api";
import InspectorFrame from "./InspectorFrame";
import MetadataFields from "./MetadataFields";
import ArtworkEditor from "./ArtworkEditor";
import MetadataEditResult from "./MetadataEditResult";
export interface MetadataEditorHandle{flush:()=>Promise<void>}
interface Context{draft:MetadataDraft;snapshot:MetadataEditorSnapshot}
const sameIds=(a:string[],b:string[])=>JSON.stringify([...a].sort())===JSON.stringify([...b].sort());

function DiscardUnsavedDialog({onCancel,onDiscard}:{onCancel:()=>void;onDiscard:()=>Promise<void>}){
 const container=useRef<HTMLElement>(null),[busy,setBusy]=useState(false);
 useEffect(()=>{const previous=document.activeElement;container.current?.querySelector<HTMLButtonElement>("button")?.focus();return()=>{if(previous instanceof HTMLElement&&previous.isConnected)previous.focus();else document.querySelector<HTMLElement>(".metadata-inspector h2")?.focus();};},[]);
 return createPortal(<div className="playlist-dialog-backdrop"><section ref={container} role="alertdialog" aria-modal="true" aria-label="Discard unsaved changes?" className="playlist-move-dialog" onKeyDown={event=>{
  if(event.key==="Escape"){event.preventDefault();event.stopPropagation();if(!busy)onCancel();}
  if(event.key==="Tab"){const buttons=[...container.current!.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")],first=buttons[0],last=buttons.at(-1);if(!buttons.length){event.preventDefault();return;}if(event.shiftKey&&document.activeElement===first){event.preventDefault();last?.focus();}else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first?.focus();}}
 }}><h2>Discard unsaved changes?</h2><p>Only local changes that could not be saved will be abandoned. Your last saved native draft stays available.</p><div className="artwork-actions"><button className="secondary" disabled={busy} onClick={onCancel}>Keep editing</button><button className="primary" disabled={busy} onClick={()=>{setBusy(true);void onDiscard().finally(()=>setBusy(false));}}>{busy?"Discarding local edits…":"Discard unsaved changes"}</button></div></section></div>,document.body);
}

function MeasuredAnalysis({snapshot}:{snapshot:MetadataEditorSnapshot}){
 if(snapshot.tracks.length!==1)return <p className="setting-note">Measured key, BPM, energy and cue timing remain associated with each selected track.</p>;
 const measured=snapshot.tracks[0].measured;
 return <section className="measured-analysis" aria-label="Measured analysis"><h3>Measured analysis <small>Read-only</small></h3><dl><div><dt>Key</dt><dd>{measured.key??"—"}</dd></div><div><dt>BPM</dt><dd>{measured.bpm!==null?measured.bpm.toFixed(2):"—"}</dd></div><div><dt>Energy</dt><dd>{measured.energy!==null?measured.energy+" / 10":"—"}</dd></div></dl></section>;
}

const MetadataForm=forwardRef<MetadataEditorHandle,{bridge:Bridge;context:Context;onClose:()=>void|Promise<void>;onPending:()=>void|Promise<void>;onChanged:()=>void;onNew:()=>void;transitioning:boolean}>(function MetadataForm({bridge,context,onClose,onPending,onChanged,onNew,transitioning},ref){
 const session=useMetadataDraft(bridge,context.draft),[preview,setPreview]=useState<MetadataEditPreview|null>(null),[result,setResult]=useState<MetadataEditBatch|null>(null),[busy,setBusy]=useState<string|null>(null),[error,setError]=useState<string|null>(null),[confirmDiscard,setConfirmDiscard]=useState(false);
 const alive=useRef(true),operation=useRef<string|null>(null),flight=useRef(false),cancelled=useRef(false),current=useRef(session);current.current=session;
 const previewRef=useRef(preview);previewRef.current=preview;
 const evidenceRevision=useRef(0),publishing=useRef(false),resultRef=useRef(result),historyRequest=useRef(0),inputs=useRef<Promise<void>>(Promise.resolve());resultRef.current=result;
 const refreshBatch=useCallback((previous:MetadataEditBatch)=>{
  const request=++historyRequest.current;
  return bridge.metadataEditBatch(previous.id).then(latest=>{if(alive.current&&request===historyRequest.current&&resultRef.current?.id===previous.id&&resultRef.current.draft_revision===previous.draft_revision&&latest.draft_revision===previous.draft_revision){resultRef.current=latest;setResult(latest);}}).catch(e=>{if(alive.current&&request===historyRequest.current)setError(current=>current??errorMessage(e));});
 },[bridge]);
 const installResult=(next:MetadataEditBatch)=>{resultRef.current=next;setResult(next);if(next.state==="queued")void refreshBatch(next);};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 useEffect(()=>{
  let active=true;
  const subscription=bridge.onLibraryChange(event=>{if(active&&(!event.track_ids.length||event.track_ids.some(id=>context.draft.track_ids.includes(id)))){evidenceRevision.current++;previewRef.current=null;setPreview(null);if(!publishing.current&&!resultRef.current)setError("Selected files changed. Preview changes again before saving.");}});
  return()=>{active=false;void subscription.then(dispose=>dispose());};
 },[bridge,context.draft.track_ids]);
 useEffect(()=>{
  let active=true;
  const subscription=bridge.onMetadataEditChange(()=>{const previous=resultRef.current;if(active&&previous)void refreshBatch(previous);});
  return()=>{active=false;historyRequest.current++;void subscription.then(dispose=>dispose());};
 },[bridge,refreshBatch]);
 const persist=async()=>{for(;;){const pending=inputs.current;await pending;await current.current.flush();if(pending===inputs.current)return;}};
 useImperativeHandle(ref,()=>({flush:persist}),[]);
 const change=(update:Parameters<typeof session.edit>[0])=>{setPreview(null);previewRef.current=null;setResult(null);resultRef.current=null;historyRequest.current++;session.edit(update);};
 const close=async()=>{await persist();await onClose();};
 const run=async(label:string,action:(id:string)=>Promise<void>)=>{
  if(flight.current)return;
  flight.current=true;cancelled.current=false;
  setBusy(label);setError(null);
  try{await inspectorOperation(bridge,async id=>{if(cancelled.current)throw Error("Operation cancelled before it started.");operation.current=id;await action(id);});}
  catch(e){if(alive.current)setError(errorMessage(e));}
  finally{flight.current=false;operation.current=null;if(alive.current)setBusy(null);}
 };
 const check=()=>run("Checking selected files…",async id=>{
  await persist();const draft=await current.current.flush(),stamp=current.current.stamp(),evidence=evidenceRevision.current;
  const next=await bridge.previewMetadataEdits(id,draft.id,draft.revision);
  if(alive.current&&stamp===current.current.stamp()&&evidence===evidenceRevision.current){setPreview(next);previewRef.current=next;}
 });
 const mutate=(label:string,action:(id:string)=>Promise<MetadataEditBatch>)=>run(label,async id=>{
  setPreview(null);previewRef.current=null;
  publishing.current=true;
  let failure:unknown;
  try{const next=await action(id);if(alive.current)installResult(next);}catch(e){
   failure=e;
   try{const known=resultRef.current;const summary=known??(await bridge.metadataEditHistoryPage(0,50)).batches.find(batch=>batch.draft_id===context.draft.id&&batch.draft_revision===current.current.draft.revision);if(summary){const latest=await bridge.metadataEditBatch(summary.id);if(alive.current)installResult(latest);}}catch{/* Preserve the publication error if history is also unavailable. */}
  }
  finally{publishing.current=false;onChanged();}
  if(failure)throw failure;
 });
 const save=()=>{const plan=previewRef.current;if(!plan||!plan.can_apply)return;void mutate("Saving metadata…",id=>bridge.applyMetadataEdits(id,plan.id));};
 useEffect(()=>{
  const key=(event:KeyboardEvent)=>{if((event.metaKey||event.ctrlKey)&&!event.altKey&&event.key.toLowerCase()==="s"){event.preventDefault();if(busy||transitioning||confirmDiscard)return;if(previewRef.current?.can_apply)save();else void check();}};
  window.addEventListener("keydown",key);return()=>window.removeEventListener("keydown",key);
 });
 const disabled=!!busy||transitioning||!!result||confirmDiscard;
 const invalid=Object.keys(session.patch.form_values??{}).length>0;
 const title=context.snapshot.tracks.length===1?context.snapshot.tracks[0].title:context.snapshot.tracks.length+" selected tracks";
 return <InspectorFrame title="Edit metadata" description={title} onClose={close} navigation={<button className="inspector-pending-link" onClick={()=>void closeToPending()}>Drafts and history</button>} actions={<><span role="status" className={session.error?"attention":"muted"}>{busy??(session.pending?"Saving draft…":session.error?"Draft not saved":result?result.state.replaceAll("_"," "):session.discarded?"Unsaved edits discarded":"Draft saved locally")}</span>{busy?<button className="secondary" onClick={()=>{cancelled.current=true;if(operation.current)void bridge.cancelMetadataOperation(operation.current).catch(e=>setError(errorMessage(e)));}}>Cancel operation</button>:result?<><button className="secondary" onClick={onNew}>New edit</button>{result.can_recover?<button className="primary" onClick={()=>void mutate("Recovering metadata…",id=>bridge.recoverMetadataEdits(id,result.id))}>Retry recovery</button>:null}{result.can_undo?<button className="secondary" onClick={()=>void mutate("Checking Undo…",id=>bridge.undoMetadataEdits(id,result.id))}><Undo2 size={14}/>Undo saved changes</button>:null}</>:<><button className="secondary" disabled={disabled||session.pending||!!session.error||invalid} onClick={()=>void check()}><ScanLine size={14}/>Preview changes</button><button className="primary" disabled={disabled||session.pending||!!session.error||!preview?.can_apply} onClick={save}><Save size={14}/>Save changes</button></>}</>}>
  {error?<div role="alert" className="inspector-error">{error}</div>:null}{session.error?<div role="alert" className="inspector-error">{session.error}<button className="secondary" onClick={session.retry}>Retry saving draft</button><button className="back-link" onClick={()=>setConfirmDiscard(true)}>Discard unsaved changes</button></div>:null}{confirmDiscard?<DiscardUnsavedDialog onCancel={()=>setConfirmDiscard(false)} onDiscard={async()=>{await session.discardUnsaved();setPreview(null);previewRef.current=null;setError(null);setConfirmDiscard(false);}}/>:null}
  <details className="metadata-paths"><summary>Selected files · {context.snapshot.tracks.length}</summary>{context.snapshot.tracks.map(track=><code key={track.track_id}>{track.display_path}</code>)}</details>
  <MeasuredAnalysis snapshot={context.snapshot}/>
  <fieldset className="metadata-form" disabled={disabled}><MetadataFields snapshot={context.snapshot} patch={session.patch} change={change} disabled={disabled}/><ArtworkEditor bridge={bridge} snapshot={context.snapshot} choices={session.patch.artwork} disabled={disabled} onPendingInput={pending=>{inputs.current=Promise.all([inputs.current,pending]).then(()=>{});}} onChange={artwork=>change(patch=>({...patch,artwork}))}/></fieldset>
  {preview?<MetadataEditResult result={preview}/>:null}{result?<MetadataEditResult result={result}/>:null}
 </InspectorFrame>;
 async function closeToPending(){try{await persist();await onPending();}catch(e){setError(errorMessage(e));}}
});

export default forwardRef<MetadataEditorHandle,{bridge:Bridge;trackIds:string[];draftId?:string|null;onClose:()=>void|Promise<void>;onPending:()=>void|Promise<void>;onChanged:()=>void;exiting?:boolean}>(function MetadataEditor({bridge,trackIds,draftId,onClose,onPending,onChanged,exiting=false},ref){
 const [context,setContext]=useState<Context|null>(null),[error,setError]=useState<string|null>(null),[transitioning,setTransitioning]=useState(false),[reload,setReload]=useState(0);
 const form=useRef<MetadataEditorHandle>(null),generation=useRef(0),alive=useRef(true),read=useRef<string|null>(null),queue=useRef(Promise.resolve()),forceNew=useRef(false);
 useImperativeHandle(ref,()=>({flush:async()=>{await form.current?.flush();}}),[]);
 const selection=JSON.stringify(trackIds);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;if(read.current)void bridge.cancelMetadataOperation(read.current).catch(()=>{});};},[bridge]);
 useEffect(()=>{
  const current=++generation.current;
  if(read.current)void bridge.cancelMetadataOperation(read.current).catch(()=>{});
  setTransitioning(true);setError(null);
  queue.current=queue.current.catch(()=>{}).then(async()=>{
   if(!alive.current||current!==generation.current)return;
   try{
    await form.current?.flush();
    if(!alive.current||current!==generation.current)return;
    setContext(null);
    if(!trackIds.length&&!draftId)return;
    const drafts=await bridge.metadataDrafts();
    let draft=forceNew.current?undefined:draftId?await bridge.metadataDraft(draftId):drafts.find(value=>!["saved","discarded"].includes(value.state)&&sameIds(value.track_ids,trackIds));
    const invoke=<T,>(call:(id:string)=>Promise<T>)=>inspectorOperation(bridge,async id=>{if(!alive.current||current!==generation.current)throw Error("superseded");read.current=id;try{return await call(id);}finally{if(read.current===id)read.current=null;}});
    if(!draft){draft=await invoke(id=>bridge.createMetadataDraft(id,trackIds));forceNew.current=false;}
    const snapshot=await invoke(id=>bridge.metadataEditorSnapshot(id,draft!.track_ids));
    if(alive.current&&current===generation.current)setContext({draft,snapshot});
   }catch(e){if(alive.current&&current===generation.current)setError(errorMessage(e));}
   finally{if(alive.current&&current===generation.current)setTransitioning(false);}
  });
 },[bridge,selection,draftId,reload]);
 const restart=useCallback(()=>setReload(value=>value+1),[]);
 if(context)return <>{error?<div role="alert" className="inspector-error">{error}<button onClick={restart}>Retry selection</button></div>:null}<MetadataForm key={context.draft.id+":"+reload} ref={form} {...{bridge,context,onClose,onPending,onChanged}} transitioning={transitioning||exiting} onNew={()=>{forceNew.current=true;restart();}}/></>;
 return <InspectorFrame title="Edit metadata" description={trackIds.length?trackIds.length+" selected tracks":"Select library tracks to edit"} onClose={onClose}><div className="inspector-empty">{transitioning?<><LoaderCircle size={24} className="spin"/><p role="status">Reading native metadata…</p></>:error?<><p role="alert">{error}</p><button className="secondary" onClick={restart}>Try again</button></>:<p>Choose one or more tracks. Playback stays independent of this selection.</p>}<button className="back-link" onClick={()=>void onPending()}>Open drafts and history</button></div></InspectorFrame>;
});
