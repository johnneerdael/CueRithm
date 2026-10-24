import {useEffect,useReducer,useRef,useState} from "react";
import {importApi,type ImportBridge} from "../import-api";
import {defaultImportPolicy,type ImportPolicy as Policy,type ImportInspection,type ImportInspectRequest,type NativeImportSelection,type SourceProvider,type ImportSourceCandidate,type ImportPage,type ImportPageQuery,type RecordSelection,type ImportPreview,type ImportPreviewPage,type ImportConflictDecision,type ImportCandidateChoice,type ImportApplyResult,type ImportEffectsPreview,type ImportEffectsPage,type ImportProcessingSummary} from "../import-types";
import type {RekordboxSyncTarget} from "../rekordbox-sync-types";
import type {OrganizationRoot} from "../organization-types";
import {nativeOrganization} from "../organization-api";
import {ImportSources} from "./ImportSources";
import {ImportSelection} from "./ImportSelection";
import {ImportPolicy} from "./ImportPolicy";
import {ImportReview} from "./ImportReview";
import {ImportHistory} from "./ImportHistory";
import {ImportEffectReview} from "./ImportEffectReview";
import type {ImportOperation} from "../import-types";
import "./LibraryImport.css";
type Step="history"|"source"|"selection"|"policy"|"review"|"result";
type Kind=NonNullable<ImportPageQuery["kind"]>;
interface State {step:Step;provider:SourceProvider;selection:NativeImportSelection|null;request:ImportInspectRequest;inspection:ImportInspection|null;page:ImportPage|null;kind:Kind;tracks:RecordSelection;collections:RecordSelection;policy:Policy;preview:ImportPreview|null;review:ImportPreviewPage|null;decisions:ImportConflictDecision[];candidateChoices:ImportCandidateChoice[];result:ImportApplyResult|null;effects:ImportEffectsPreview|null;effectPage:ImportEffectsPage|null;processing:ImportProcessingSummary|null}
function initial(sourceId?:string):State {return {step:"source",provider:"m3u",selection:null,request:{selection_id:"",binding:sourceId?{choice:"replica",source_id:sourceId}:{choice:"automatic"},capture_file_tags:false,include_history:false,include_queue:false,path_mappings:[]},inspection:null,page:null,kind:"track",tracks:{selection:"all"},collections:{selection:"all"},policy:defaultImportPolicy(),preview:null,review:null,decisions:[],candidateChoices:[],result:null,effects:null,effectPage:null,processing:null};}
type Action={type:"patch";value:Partial<State>}|{type:"reset";sourceId?:string};
function reducer(state:State,action:Action):State{return action.type==="reset"?initial(action.sourceId):{...state,...action.value};}
function failure(error:unknown):{code:string;message:string}{if(error&&typeof error==="object"&&"message" in error)return {code:"code" in error?String(error.code):"operation_failed",message:String(error.message)};return {code:"operation_failed",message:String(error)};}
export function LibraryImport({initialSourceId,onClose,api=importApi}:{initialSourceId?:string;onClose:()=>void;api?:ImportBridge}) {
 const [state,dispatch]=useReducer(reducer,initialSourceId,initial);
 const [candidates,setCandidates]=useState<ImportSourceCandidate[]>([]),[targets,setTargets]=useState<RekordboxSyncTarget[]>([]),[targetId,setTargetId]=useState("");
 const [mappingDisplay,setMappingDisplay]=useState<Record<string,string>>({});
 const [root,setRoot]=useState<OrganizationRoot|null>(null),[busy,setBusy]=useState<string|null>(null),[error,setError]=useState<{code:string;message:string}|null>(null);
 const refreshStarted=useRef(false);
 const mounted=useRef(true),active=useRef<string|null>(null),prepared=useRef<string|null>(null),discarding=useRef(Promise.resolve()),generation=useRef(0);
 const patch=(value:Partial<State>)=>{if(mounted.current)dispatch({type:"patch",value});};
 const discard=()=>{const id=prepared.current;prepared.current=null;if(id){discarding.current=discarding.current.then(()=>api.discardImportPreview(id)).catch(()=>{});}return discarding.current;};
 useEffect(()=>{mounted.current=true;void api.discoverImportSources().then(next=>{if(mounted.current)setCandidates(next);}).catch(e=>{if(mounted.current)setError(failure(e));});return()=>{mounted.current=false;generation.current++;if(active.current)void api.cancel(active.current).catch(()=>{});void discard();};},[api]);
 useEffect(()=>{if(state.provider!=="rekordbox")return;let cancelled=false;void api.discoverRekordboxSources().then(next=>{if(!cancelled)setTargets(next);}).catch(e=>{if(!cancelled)setError(failure(e));});return()=>{cancelled=true;};},[state.provider,api]);
 const run=async(label:string,work:(id:string)=>Promise<void>)=>{
  if(active.current)return;const id=crypto.randomUUID();active.current=id;setBusy(label);setError(null);
  try{await work(id);}catch(e){if(mounted.current)setError(failure(e));}finally{active.current=null;if(mounted.current)setBusy(null);}
 };
 const refreshSource=(sourceId:string,savedPolicy?:Policy)=>void run("Refreshing source",async id=>{
  await discard();const policy=savedPolicy??(await api.importOperationPage({source_id:sourceId,limit:1})).records[0]?.policy??defaultImportPolicy();
  const inspection=await api.inspectImportRefresh(id,sourceId);const page=await api.importPage(inspection.inspection_id,{kind:"track",limit:100});
  patch({...initial(sourceId),step:"selection",provider:inspection.profile.provider,inspection,page,policy});
 });
 useEffect(()=>{if(initialSourceId&&!refreshStarted.current){refreshStarted.current=true;refreshSource(initialSourceId);}},[initialSourceId]);
 const source=(provider:SourceProvider)=>{generation.current++;void discard();setError(null);dispatch({type:"reset"});patch({provider});};
 const choose=()=>void run("Choosing library",async id=>{const selected=state.provider==="rekordbox"?await api.chooseRekordboxImportSource(id,targetId):await api.chooseImportSource(id,state.provider);if(selected)patch({selection:selected,request:{...state.request,selection_id:selected.selection_id}});});
 const inspect=()=>void run("Inspecting library",async id=>{
  await discard();const current=++generation.current;const inspection=await api.inspectImport(id,state.request);const page=await api.importPage(inspection.inspection_id,{kind:"track",limit:100});
  if(current===generation.current)patch({step:"selection",inspection,page,kind:"track",preview:null,review:null,policy:{...state.policy,capture_file_tags:state.request.capture_file_tags,include_history:state.request.include_history,include_queue:state.request.include_queue}});
 });
 const records=(kind:Kind,cursor:string|null=null)=>void run("Loading source records",async()=>{if(!state.inspection)return;const page=await api.importPage(state.inspection.inspection_id,{kind,cursor,limit:100});patch({kind,page});});
 const policy=(value:Policy)=>{void discard();patch({policy:value,preview:null,review:null});};
 const review=()=>void run("Building import preview",async id=>{
  if(!state.inspection)return;await discard();const current=++generation.current;
  const preview=await api.previewImport(id,{inspection_id:state.inspection.inspection_id,tracks:state.tracks,collections:state.collections,policy:state.policy,target_items:{},decisions:state.decisions,candidate_choices:state.candidateChoices.filter(choice=>state.tracks.selection==="all"||state.tracks.ids.includes(choice.external_id))});
  if(!mounted.current||current!==generation.current){await api.discardImportPreview(preview.preview_id);return;}
  prepared.current=preview.preview_id;const page=await api.importPreviewPage(preview.preview_id,{limit:100});patch({step:"review",preview,review:page});
 });
 const decide=(decision:ImportConflictDecision)=>{void discard();patch({step:"policy",preview:null,review:null,decisions:[...state.decisions.filter(d=>d.external_id!==decision.external_id||d.field!==decision.field),decision]});};
 const apply=()=>void run("Importing reviewed records",async id=>{if(!state.preview||prepared.current!==state.preview.preview_id)return;const result=await api.applyImport(id,state.preview.preview_id);prepared.current=null;patch({step:"result",result});});
 const process=()=>void run("Generating selected processing proposals",async id=>{if(!state.result)return;const processing=await api.runImportProcessing(id);patch({processing});});
 const effects=(operation?:ImportOperation)=>void run("Building effects review",async id=>{
  const result=operation?.result??state.result;if(!result)return;await discard();const effects=await api.previewImportEffects(id,result.operation_id);
  if(!mounted.current){await api.discardImportPreview(effects.preview_id);return;}
  prepared.current=effects.preview_id;const effectPage=await api.importEffectsPage(effects.preview_id,{limit:100});patch({step:"result",result,effects,effectPage});
 });
 const applyEffects=()=>void run("Applying reviewed processing effects",async id=>{if(!state.effects||prepared.current!==state.effects.preview_id)return;const processing=await api.applyImportEffects(id,state.effects.preview_id);prepared.current=null;patch({processing,effects:null,effectPage:null});});
 const chooseRoot=()=>void run("Choosing naming folder",async()=>{const chosen=await nativeOrganization.chooseOrganizationRoot();if(chosen&&mounted.current){setRoot(chosen);if(state.policy.relocation.mode==="copy"||state.policy.relocation.mode==="move")policy({...state.policy,relocation:{...state.policy.relocation,root_id:chosen.id}});}});
 return <section className="library-import" aria-label="Library import">
  <header className="view-title"><div><div className="eyebrow">DJ LIBRARIES</div><h1>Import library</h1><p>Keep your imported values. Choose any further processing independently.</p><span className="import-step">{state.step}</span></div><div><button type="button" disabled={!!busy} onClick={()=>{void discard();patch({step:state.step==="history"?"source":"history",preview:null,effects:null});}}>{state.step==="history"?"New import":"Import history"}</button><button type="button" disabled={!!busy} onClick={onClose}>Close import</button></div></header>
  {error?<div className="error-banner" role="alert"><p>{error.message}</p>{["stale_preview","stale_cursor","unsupported_profile"].includes(error.code)?<button type="button" disabled={!!busy} onClick={()=>{void discard();patch({step:"source",preview:null,review:null});setError(null);}}>Inspect again</button>:null}</div>:null}
  {busy?<div className="import-status"><p role="status">{busy}…</p><button type="button" onClick={()=>{if(active.current)void api.cancel(active.current).catch(e=>setError(failure(e)));}}>Cancel import operation</button></div>:null}
  {state.step==="history"?<ImportHistory api={api} onRefresh={refreshSource} onReview={effects} busy={!!busy}/>:null}
  {state.step==="source"?<ImportSources provider={state.provider} onProvider={source} candidates={candidates} selection={state.selection} request={state.request} onRequest={request=>patch({request})} onChoose={choose} onAddArtifacts={()=>void run("Selecting additional native artifacts",async id=>{if(!state.selection)return;const selected=await api.addImportArtifacts(id,state.selection.selection_id);if(selected)patch({selection:selected,request:{...state.request,selection_id:selected.selection_id}});})} onChooseMapping={()=>void run("Choosing local mapping folder",async()=>{const mapping=await api.chooseImportMapping();if(mapping){setMappingDisplay(previous=>({...previous,[mapping.root_id]:mapping.display_location}));patch({request:{...state.request,path_mappings:[...state.request.path_mappings,{source_prefix:"",root_id:mapping.root_id}]}});}})} mappingDisplay={mappingDisplay} onInspect={inspect} rekordboxTargets={targets} targetId={targetId} onTarget={setTargetId} busy={!!busy}/>:null}
  {state.step==="selection"&&state.inspection?<><ImportSelection inspection={state.inspection} page={state.page} kind={state.kind} selected={state.kind==="collection"?state.collections:state.tracks} onSelection={selected=>patch(state.kind==="collection"?{collections:selected}:{tracks:selected})} onKind={kind=>records(kind)} onNext={()=>records(state.kind,state.page?.next_cursor)} busy={!!busy} candidateChoices={state.candidateChoices} onCandidateChoice={choice=>patch({candidateChoices:[...state.candidateChoices.filter(c=>c.external_id!==choice.external_id||c.field!==choice.field),...(choice.candidate_id?[choice]:[])]})}/><div className="import-actions"><button type="button" disabled={!!busy} onClick={()=>patch({step:"source"})}>Change source</button><button type="button" className="primary" disabled={!!busy} onClick={()=>patch({step:"policy"})}>Processing choices</button></div></>:null}
  {state.step==="policy"&&state.inspection?<><fieldset disabled={!!busy}><ImportPolicy value={state.policy} onChange={policy} profile={state.inspection.profile} onChooseRoot={chooseRoot} root={root}/></fieldset><div className="import-actions"><button type="button" disabled={!!busy} onClick={()=>patch({step:"selection"})}>Back to selection</button><button type="button" className="primary" disabled={!!busy} onClick={review}>Review import</button></div></>:null}
  {state.step==="review"&&state.preview?<><ImportReview preview={state.preview} page={state.review} busy={!!busy} onNext={()=>void run("Loading comparisons",async()=>{const page=await api.importPreviewPage(state.preview!.preview_id,{cursor:state.review?.next_cursor,limit:100});patch({review:page});})} onDecision={decide} onApply={apply}/><button type="button" disabled={!!busy} onClick={()=>{void discard();patch({step:"policy",preview:null,review:null});}}>Change choices</button></>:null}
  {state.step==="result"&&state.result?<div><h2>Import result</h2><p role="status">{state.result.tracks_created} {state.result.tracks_created===1?"track imported":"tracks imported"}. {state.result.fields_changed} fields changed; {state.result.collections_changed} collections changed.</p><p>Imported tracks are visible in Tracks. Missing media remains visible and can be resolved later.</p>
   {state.result.processing_requested?<div className="import-actions"><button type="button" disabled={!!busy||!!state.effects} onClick={process}>Generate selected processing</button><button type="button" disabled={!!busy||!state.processing||!!state.effects} onClick={()=>effects()}>Review processing effects</button></div>:<p>Stored as imported. No optional processing was requested.</p>}
   {state.processing?<p>{state.processing.processed} processed · {state.processing.reused} reused · {state.processing.unchanged} unchanged · {state.processing.blocked} blocked · {state.processing.failed} failed · {state.processing.cancelled} cancelled · {state.processing.recovery_required} recovery required</p>:null}
   {state.effects?<div><h3>Review actual processing effects</h3><p>{state.effects.ready} ready · {state.effects.blocked} blocked · {state.effects.unchanged} unchanged</p>{state.effectPage?.records.map((effect,index)=><ImportEffectReview key={`${effect.item_id}-${effect.stage}-${index}`} effect={effect}/>)}{state.effectPage?.next_cursor?<button type="button" disabled={!!busy} onClick={()=>void run("Loading effects",async()=>{const effectPage=await api.importEffectsPage(state.effects!.preview_id,{cursor:state.effectPage?.next_cursor,limit:100});patch({effectPage});})}>Next effects page</button>:null}<button type="button" className="primary" disabled={!!busy||!state.effectPage} onClick={applyEffects}>Apply processing effects</button></div>:null}
   <button type="button" disabled={!!busy} onClick={()=>{void discard();dispatch({type:"reset"});}}>Import another library</button>
  </div>:null}
 </section>;
}
