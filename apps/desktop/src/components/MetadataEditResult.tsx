import type {MetadataEditPreview,MetadataEditBatch,MetadataValues} from "../editor-types";
import {dateText} from "../editor-fields";
function value(values:MetadataValues,field:string):string{
 const key=field.replace(/^text\./,"");
 if(key==="id3_version")return values.capabilities.id3_version??"Unknown";
 if(key in values.text)return values.text[key as keyof typeof values.text]?.join(" · ")||"Empty";
 if(key==="release_date"||key==="original_date")return dateText(values[key])||"Empty";
 if(key.includes("artwork"))return values.artwork.length+" picture(s)";
 if(key.includes("rating"))return values.ratings.map(r=>r.identity.owner+": "+(r.stars??"unrated")).join(" · ")||"No rating";
 if(key.includes("comment"))return values.comments.map(comment=>comment.values.join(" · ")).join(" / ")||"Empty";
 const other=values[key as keyof MetadataValues];return other===null||other===undefined?"Empty":typeof other==="object"?"Native record":String(other);
}
export default function MetadataEditResult({result}:{result:MetadataEditPreview|MetadataEditBatch}){
 const batch="state" in result;
 return <section className="metadata-preview" aria-label={batch?"Metadata publication result":"Metadata changes preview"}><h3>{batch?"Publication · "+result.state.replaceAll("_"," "):"Review changes · "+result.rows.length+" tracks"}</h3>{batch&&result.error?<p role="alert">{result.error}</p>:null}{result.rows.map(row=><details key={row.track_id} open={result.rows.length===1||!!row.error}><summary><strong>{row.title}</strong><span className={row.error?"attention":"muted"}>{row.outcome.replaceAll("_"," ")}</span></summary>{row.error?<p role="alert" className="metadata-field-error">{row.error}</p>:null}{row.fields.map((field,index)=><div className="metadata-diff" key={field.field+index}><strong>{field.field.replaceAll("_"," ").replace(/^text\./,"")}</strong><span><small>Before</small>{value(row.before,field.field)}</span><span><small>After</small>{value(row.after,field.field)}</span><small className={field.reason?"attention":"muted"}>{field.status}{field.reason?" · "+field.reason:""}</small></div>)}</details>)}</section>;
}
