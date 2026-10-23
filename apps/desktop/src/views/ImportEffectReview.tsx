import type {ImportEffect} from "../import-types";

const labels:Record<string,string>={"analysis.key":"Key analysis","analysis.energy":"Energy analysis","analysis.bpm":"BPM analysis","analysis.beatgrid":"Beatgrid analysis","generation.cues":"Generated hotcues","metadata.enrichment":"Metadata enrichment","metadata.cleanup":"Cleanup","file.tags":"Audio tags","file.relocation":"File naming"};
const object=(value:unknown):Record<string,unknown>=>value!==null&&typeof value==="object"&&!Array.isArray(value)?value as Record<string,unknown>:{};
const rows=(value:unknown)=>Array.isArray(value)?value.map(object):[];
const display=(value:unknown)=>value===undefined?"No value":typeof value==="string"?value:JSON.stringify(value,null,2);
export function ImportEffectReview({effect}:{effect:ImportEffect}) {
 const changes=object(effect.changes),before=object(changes.before);
 return <article className="import-effect-review"><h4>{labels[effect.stage]??effect.stage}: {effect.outcome}</h4>{effect.reason?<p>{effect.reason}</p>:null}
  {Object.entries(object(changes.fields)).map(([field,value])=><div key={field}><strong>{field.replace(/^metadata\./,"").replaceAll("_"," ")}</strong><dl><dt>Before</dt><dd><pre>{display(before[field])}</pre></dd><dt>After</dt><dd><pre>{display(value)}</pre></dd></dl></div>)}
  {rows(changes.audio_tags).map((row,index)=><div key={index}><p>{String(row.path??row.title??"Selected audio file")}</p><dl><dt>Tags before</dt><dd><pre>{display(row.before)}</pre></dd><dt>Tags after</dt><dd><pre>{display(row.after)}</pre></dd></dl></div>)}
  {rows(changes.files).map((row,index)=><div key={index}><dl><dt>Current file</dt><dd>{String(row.before_path??"")}</dd><dt>Destination</dt><dd>{String(row.after_path??"No file change")}</dd></dl>{row.reason?<p>{String(row.reason)}</p>:null}</div>)}
 </article>;
}
