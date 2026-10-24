import {useState} from "react";
import type {ImportPreview,ImportPreviewPage,ImportConflictDecision,ImportFieldEffect,Json} from "../import-types";
import {displayImportedValue} from "./ImportSelection";
export function ImportReview({preview,page,busy,onNext,onDecision,onApply}:{preview:ImportPreview;page:ImportPreviewPage|null;busy:boolean;onNext:()=>void;onDecision:(decision:ImportConflictDecision)=>void;onApply:()=>void}){
 return <div><h2>Review import</h2><p>{preview.tracks_created} new tracks · {preview.fields_changed} field changes · {preview.collections_changed} collection changes · {preview.conflicts} conflicts · {preview.blocked} blocked</p>
  <p>{preview.processing_requested?"Selected processing generates proposals for a separate effects review after import.":"Store as imported. No analysis, retagging, tag writes or renaming requested."}</p>
  <div className="import-records">{page?.effects.map(effect=><article key={`${effect.kind}-${effect.external_id}`}><strong>{effect.external_id}</strong>{effect.fields.map(field=><div className="import-field-review" key={field.field}><strong>{field.field}</strong><dl><dt>Source</dt><dd>{displayImportedValue(field.source)}</dd><dt>Local</dt><dd>{displayImportedValue(field.local)}</dd><dt>Proposed</dt><dd>{displayImportedValue(field.proposed)}</dd></dl>{field.blocked_reason?<p role="alert">{field.blocked_reason}</p>:null}{field.conflict?<Conflict field={field} externalId={effect.external_id} busy={busy} onDecision={onDecision}/>:null}</div>)}</article>)}</div>
  {page?.next_cursor?<button type="button" disabled={busy} onClick={onNext}>Next comparison page</button>:null}
  <button type="button" className="primary" disabled={busy||!page} onClick={onApply}>Apply import</button>
 </div>;
}

function Conflict({field,externalId,busy,onDecision}:{field:ImportFieldEffect;externalId:string;busy:boolean;onDecision:(decision:ImportConflictDecision)=>void}){
 const [choice,setChoice]=useState("keep_local"),[custom,setCustom]=useState(""),[error,setError]=useState<string|null>(null);
 return <div><label>Conflict decision<select value={choice} disabled={busy} onChange={e=>{const choice=e.target.value;setChoice(choice);setError(null);if(choice!=="custom")onDecision({external_id:externalId,field:field.field,decision:{choice:choice as "keep_local"|"use_source"}});}}><option value="keep_local">Keep local</option><option value="use_source">Use source — rebuild preview</option><option value="custom">Custom value — review JSON</option></select></label>{choice==="custom"?<><p>Supply the complete typed value for this field or atomic cue/grid family. The backend validates it in a new preview.</p><label>Custom value JSON<textarea value={custom} onChange={e=>setCustom(e.target.value)} disabled={busy}/></label>{error?<p role="alert">{error}</p>:null}<button type="button" disabled={busy||!custom.trim()} onClick={()=>{try{const value:Json=JSON.parse(custom);onDecision({external_id:externalId,field:field.field,decision:{choice:"custom",value}});}catch{setError("Enter valid JSON before rebuilding the preview.");}}}>Use custom value — rebuild preview</button></>:null}</div>;
}
