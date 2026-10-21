import {useState} from "react";
import type {NativeFieldDecision,NativeFieldObservation,RekordboxDecisions,RekordboxSyncFieldReview} from "../rekordbox-sync-types";
import {rekordboxMetadataFields} from "../rekordbox-sync-types";

export function equalObservations(field:RekordboxSyncFieldReview){return field.local.state==="present"&&field.remote.state==="present"&&JSON.stringify(field.local.value)===JSON.stringify(field.remote.value);}
function object(value:unknown):Record<string,unknown>|null{return value!==null&&typeof value==="object"&&!Array.isArray(value)?value as Record<string,unknown>:null;}
function number(value:unknown){return typeof value==="number"&&Number.isFinite(value)?value:null;}
function clock(seconds:number){const millis=Math.round(Math.max(0,seconds)*1000);return `${Math.floor(millis/60000)}:${String(Math.floor(millis/1000)%60).padStart(2,"0")}.${String(millis%1000).padStart(3,"0")}`;}
function cueText(value:unknown){
 const cue=object(value);if(!cue)return "Cue details unavailable";
 const data=object(cue.data),rekordbox=object(data?.rekordbox);
 const memory=cue.memory===true||rekordbox?.originalNum===-1;
 const position=number(cue.slot)??number(cue.position)??number(cue.index);
 const start=number(cue.start_seconds)??number(cue.startTime),end=number(cue.end_seconds)??number(cue.endTime);
 const loop=end!==null||cue.type==="5";
 const label=memory?"Memory":position!==null&&position>=0&&position<8?`Hot ${String.fromCharCode(65+position)}`:"Cue";
 const name=typeof cue.name==="string"?cue.name:typeof cue.comment==="string"?cue.comment:"";
 const active=cue.active_loop===true||cue.activeLoop===true;
 return `${label}${loop?" loop":""}${name?` · ${name}`:""} · ${start===null?"Time unavailable":clock(start)}${end!==null?` – ${clock(end)}`:""}${active?" · Active loop":""}`;
}
function gridText(value:unknown){
 const entries=Array.isArray(value)?value:[value];if(!entries.length)return "No beatgrid anchors";
 const points=entries.map(object).filter((entry):entry is Record<string,unknown>=>entry!==null);
 if(!points.length)return "Beatgrid present · details unavailable";
 const native=points.some(point=>number(point.time_ms)!==null),count=entries.length;
 const bpms=points.map(point=>number(point.bpm)??(number(point.tempo)!==null?Number(point.tempo)/100:null)).filter((bpm):bpm is number=>bpm!==null);
 const low=bpms.reduce<number|null>((minimum,bpm)=>minimum===null?bpm:Math.min(minimum,bpm),null),high=bpms.reduce<number|null>((maximum,bpm)=>maximum===null?bpm:Math.max(maximum,bpm),null);
 const tempo=low===null?"Tempo unavailable":`${low.toFixed(2)}${high!==low?`–${high!.toFixed(2)}`:""} BPM`;
 const anchors=points.slice(0,native?1:5).map(point=>{const seconds=number(point.seconds)??number(point.startTime)??(number(point.time_ms)!==null?Number(point.time_ms)/1000:null);const beat=number(point.beat)??number(point.number);return `${beat!==null?`Beat ${beat}`:"Anchor"} at ${seconds===null?"unknown time":clock(seconds)}`;});
 return `${tempo} · ${count.toLocaleString()} ${native?(count===1?"beat":"beats"):(count===1?"anchor":"anchors")}\n${anchors.join("\n")}${!native&&count>5?`\n${count-5} more anchors`:""}`;
}
export function rekordboxValueText(field:string,observation:NativeFieldObservation):string {
 if(observation.state==="unavailable")return "Unavailable";
 const value=observation.value;if(value===null||value==="")return "Empty";
 if(field==="artwork")return "Artwork available";
 if(field==="cues"){if(!Array.isArray(value))return "Cues present · details unavailable";return value.length?`${value.slice(0,20).map(cueText).join("\n")}${value.length>20?`\n${value.length-20} more cues`:""}`:"No cues";}
 if(field==="beat_grid")return gridText(value);
 if(Array.isArray(value)){if(value.every(item=>typeof item==="string"))return value.filter(Boolean).join("; ")||"Empty";return `${value.length.toLocaleString()} items · details unavailable`;}
 if(typeof value==="string"||typeof value==="number"||typeof value==="boolean")return String(value);
 return "Present · details unavailable";
}
export function allowedDecisions(field:RekordboxSyncFieldReview):[NativeFieldDecision,string][]{
 const choices:[NativeFieldDecision,string][]=[];
 if(field.local.state==="present"&&field.local.value!==null)choices.push(["use_local",field.field==="cues"?"Replace listed rekordbox cues":"Use CueRithm"]);
 if(field.remote.state==="present")choices.push(["use_remote","Keep rekordbox"]);
 if(equalObservations(field))choices.push(["accept_equal","Accept equal values"]);
 if(field.remote.state==="present"&&(rekordboxMetadataFields.includes(field.field)||field.field==="cues"||field.field==="artwork"))choices.push(["clear_remote",field.field==="cues"?"Clear listed rekordbox cues":"Clear in rekordbox"]);
 choices.push(["skip","Skip this field"]);return choices;
}
const reasons={observation_unavailable:"Value unavailable",initial_acknowledgement:"First sync: choose explicitly",concurrent_changes:"Changed in both libraries",ownership:"Changed outside its owner",unowned:"Choose which library to use"};
export default function RekordboxConflicts({fields,decisions,disabled,onDecision,onBulk}:{fields:RekordboxSyncFieldReview[];decisions:RekordboxDecisions;disabled:boolean;onDecision:(field:RekordboxSyncFieldReview,decision:NativeFieldDecision|"")=>void;onBulk:(fields:RekordboxSyncFieldReview[],decision:NativeFieldDecision)=>void}){
 const [limit,setLimit]=useState(100);
 const equal=fields.filter(equalObservations),conflicts=fields.filter(f=>f.result.state==="conflict"&&!decisions[f.track_id]?.[f.field]),skipped=fields.filter(f=>decisions[f.track_id]?.[f.field]==="skip");
 const local=conflicts.filter(field=>allowedDecisions(field).some(([decision])=>decision==="use_local")),remote=conflicts.filter(field=>allowedDecisions(field).some(([decision])=>decision==="use_remote"));
 const cueFields=local.filter(field=>field.field==="cues").length;
 return <section className="rekordbox-review" aria-label="rekordbox field review">
  <div className="rekordbox-section-heading"><div><h2>Review fields</h2><p>{conflicts.length.toLocaleString()} {conflicts.length===1?"field needs":"fields need"} a choice · {skipped.length.toLocaleString()} skipped</p></div><div className="rekordbox-bulk"><button className="secondary" disabled={disabled||!equal.length} onClick={()=>onBulk(equal,"accept_equal")}>Accept equal values</button><button className="secondary" disabled={disabled||!local.length} onClick={()=>onBulk(local,"use_local")}>Use CueRithm for unresolved{cueFields?` · ${cueFields} ${cueFields===1?"cue set":"cue sets"}`:""}</button><button className="secondary" disabled={disabled||!remote.length} onClick={()=>onBulk(remote,"use_remote")}>Keep rekordbox for unresolved</button><button className="secondary" disabled={disabled||!conflicts.length} onClick={()=>onBulk(conflicts,"skip")}>Skip unresolved fields</button></div></div>
  <p className="sync-note">Choose explicitly for the first sync. Empty and unavailable values never mean clear. Skipped fields remain unsynced.</p>
  {fields.some(field=>field.field==="cues")?<p className="sync-note">Replacing or clearing listed rekordbox cues transfers their management to CueRithm after the sync commits. Unknown cue data stays protected.{cueFields?` Use CueRithm for unresolved includes ${cueFields} ${cueFields===1?"cue set":"cue sets"} and replaces their listed rekordbox cues.`:""}</p>:null}
  <div className="rekordbox-table-wrap"><table className="rekordbox-table"><thead><tr><th>Track / field</th><th>CueRithm</th><th>rekordbox</th><th>Choice</th></tr></thead><tbody>{fields.slice(0,limit).map(field=>{const decision=decisions[field.track_id]?.[field.field]??"";return <tr key={`${field.track_id}:${field.field}`}><td><strong>{field.title||"Untitled track"}</strong><small>{field.field.replaceAll("_"," ")}</small><small>{decision==="skip"?"Skipped · not acknowledged":decision?"Choice selected":field.result.state==="conflict"?reasons[field.result.reason]:field.result.state==="unchanged"?"Unchanged":"Change proposed"}</small></td><td>{rekordboxValueText(field.field,field.local)}</td><td>{rekordboxValueText(field.field,field.remote)}</td><td><select aria-label={`Choice for ${field.title} ${field.field}`} disabled={disabled} value={decision} onChange={event=>onDecision(field,event.target.value as NativeFieldDecision|"")}><option value="">Choose a value</option>{allowedDecisions(field).map(([value,label])=><option key={value} value={value}>{label}</option>)}</select></td></tr>;})}</tbody></table></div>
  {fields.length>limit?<button className="secondary" onClick={()=>setLimit(value=>value+100)}>Show {Math.min(100,fields.length-limit)} more fields</button>:null}
 </section>;
}
