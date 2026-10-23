import type {ImportPreview,ImportPreviewPage,ImportConflictDecision,ImportDecision} from "../import-types";
import {displayImportedValue} from "./ImportSelection";
export function ImportReview({preview,page,busy,onNext,onDecision,onApply}:{preview:ImportPreview;page:ImportPreviewPage|null;busy:boolean;onNext:()=>void;onDecision:(decision:ImportConflictDecision)=>void;onApply:()=>void}){
 return <div><h2>Review import</h2><p>{preview.tracks_created} new tracks · {preview.fields_changed} field changes · {preview.collections_changed} collection changes · {preview.conflicts} conflicts · {preview.blocked} blocked</p>
  <p>{preview.processing_requested?"Selected processing generates proposals for a separate effects review after import.":"Store as imported. No analysis, retagging, tag writes or renaming requested."}</p>
  <div className="import-records">{page?.effects.map(effect=><article key={`${effect.kind}-${effect.external_id}`}><strong>{effect.external_id}</strong>{effect.fields.map(field=><div className="import-field-review" key={field.field}><strong>{field.field}</strong><dl><dt>Source</dt><dd>{displayImportedValue(field.source)}</dd><dt>Local</dt><dd>{displayImportedValue(field.local)}</dd><dt>Proposed</dt><dd>{displayImportedValue(field.proposed)}</dd></dl>{field.blocked_reason?<p role="alert">{field.blocked_reason}</p>:null}{field.conflict?<label>Conflict decision<select defaultValue="keep_local" disabled={busy} onChange={e=>onDecision({external_id:effect.external_id,field:field.field,decision:{choice:e.target.value as "keep_local"|"use_source"} as ImportDecision})}><option value="keep_local">Keep local</option><option value="use_source">Use source — rebuild preview</option></select></label>:null}</div>)}</article>)}</div>
  {page?.next_cursor?<button type="button" disabled={busy} onClick={onNext}>Next comparison page</button>:null}
  <button type="button" className="primary" disabled={busy||!page} onClick={onApply}>Apply import</button>
 </div>;
}
