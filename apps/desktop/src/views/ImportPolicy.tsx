import type {ImportPolicy as Policy, SourceProfile, FieldGroup} from "../import-types";
import {defaultFields,processingRequested} from "../import-types";
import type {OrganizationRoot} from "../organization-types";

const words=(value:string)=>value.replaceAll("_"," ");
const list=(text:string)=>[...new Set(text.split(",").map(v=>v.trim()).filter(Boolean))];
const tagFields=["title","artist","album","album_artist","genre","label","version","remixer","producer","composer","lyricist","grouping","comment","catalog_number","isrc","key","bpm"];
export function ImportPolicy({value,onChange,profile,onChooseRoot,root}:{value:Policy;onChange:(value:Policy)=>void;profile:SourceProfile;onChooseRoot?:()=>void;root?:OrganizationRoot|null}) {
 const update=<K extends keyof Policy>(key:K,next:Policy[K])=>onChange({...value,[key]:next});
 const field=(group:FieldGroup,enabled:boolean)=>update("selected_fields",enabled?[...new Set([...value.selected_fields,group])]:value.selected_fields.filter(f=>f!==group));
 const analysis=(key:keyof Policy["analysis_fields"],enabled:boolean)=>{const fields={...value.analysis_fields};if(enabled)fields[key]="fill_missing";else delete fields[key];update("analysis_fields",fields);};
 const relocation=value.relocation;
 return <div className="import-policy">
  <h2>Processing choices</h2><p><strong>{processingRequested(value)?"Store imported values and run only the selected stages":"Store as imported"}</strong>. Incoming settings do not change these choices.</p>
  <fieldset><legend>Values to import</legend><p>These field choices also gate later analysis and metadata publication. Existing local conflicts keep local values until reviewed.</p>
   <div className="import-check-grid">{defaultFields.map(group=>{const cap=profile.fields[group];return <div key={group}><label><input type="checkbox" aria-label={`Import ${words(group)}`} checked={value.selected_fields.includes(group)} disabled={!cap?.read} onChange={e=>field(group,e.target.checked)}/>{words(group)}</label>{!cap?.read?<small>{cap?.reason??"This source does not expose this field"}</small>:!cap.lossless?<small>{cap.reason??"Some source details cannot be adopted"}</small>:null}</div>;})}</div>
   <label><input type="checkbox" checked={value.descriptive_fields===null} onChange={e=>update("descriptive_fields",e.target.checked?null:[])}/>All descriptive fields</label>
   {value.descriptive_fields!==null?<label>Descriptive fields to adopt<input value={value.descriptive_fields.join(", ")} onChange={e=>update("descriptive_fields",list(e.target.value))} placeholder="title, artist, album, genre"/></label>:null}
  </fieldset>
  <fieldset><legend>Optional analysis</legend><p>Imported key, energy and cues stay in place unless their adoption rules authorize a change. Private prerequisites for generating cues are not adopted automatically.</p>
   {([ ["key","Analyse key"],["energy","Analyse energy"],["bpm","Analyse BPM"],["beatgrid","Analyse beatgrid"] ] as const).map(([key,label])=><div className="import-option" key={key}><label><input type="checkbox" checked={!!value.analysis_fields[key]} onChange={e=>analysis(key,e.target.checked)}/>{label}</label>{value.analysis_fields[key]?<label>{label} adoption<select value={value.analysis_fields[key]} onChange={e=>update("analysis_fields",{...value.analysis_fields,[key]:e.target.value as "fill_missing"|"replace_selected"})}><option value="fill_missing">Fill missing only</option><option value="replace_selected">Replace selected values</option></select></label>:null}</div>)}
   <label><input type="checkbox" checked={value.cue_generation==="generate"} onChange={e=>update("cue_generation",e.target.checked?"generate":"off")}/>Generate cues</label>
   <label><input type="checkbox" checked={value.replace_hotcues} onChange={e=>update("replace_hotcues",e.target.checked)}/>Replace existing hotcues</label><small>Replacement alone generates nothing. Unknown native pad occupancy can block cue adoption; the effects review explains each item.</small>
  </fieldset>
  <fieldset><legend>Optional metadata processing</legend>
   <label><input type="checkbox" checked={value.metadata_enrichment.enabled} onChange={e=>update("metadata_enrichment",{...value.metadata_enrichment,enabled:e.target.checked})}/>Retag / enrich metadata</label>
   {value.metadata_enrichment.enabled?<><label>Metadata providers<input value={value.metadata_enrichment.providers.join(", ")} placeholder="beatport, traxsource" onChange={e=>update("metadata_enrichment",{...value.metadata_enrichment,providers:list(e.target.value)})}/></label><label>Fields to enrich<input value={value.metadata_enrichment.fields.join(", ")} placeholder="title, artist, genre, label" onChange={e=>update("metadata_enrichment",{...value.metadata_enrichment,fields:list(e.target.value)})}/></label><small>Uses configured catalog access. Missing credentials block this stage and leave the imported track visible.</small></>:null}
   <label><input type="checkbox" checked={value.cleanup.enabled} onChange={e=>update("cleanup",{...value.cleanup,enabled:e.target.checked})}/>Apply cleanup rules</label>
   {value.cleanup.enabled?<label>Cleanup rule IDs<input value={value.cleanup.rules.join(", ")} onChange={e=>update("cleanup",{...value.cleanup,rules:list(e.target.value)})}/></label>:null}
   <label><input type="checkbox" checked={value.write_audio_tags.length>0} onChange={e=>update("write_audio_tags",e.target.checked?["title","artist"]:[])}/>Write audio tags</label>
   {value.write_audio_tags.length>0?<div className="import-check-grid">{tagFields.map(name=><label key={name}><input type="checkbox" checked={value.write_audio_tags.includes(name)} onChange={e=>update("write_audio_tags",e.target.checked?[...value.write_audio_tags,name]:value.write_audio_tags.filter(f=>f!==name))}/>{`Write ${words(name)}`}</label>)}</div>:null}
   <small>Library-only enrichment does not write your files. Tag writing can use accepted imported values without a lookup.</small>
  </fieldset>
  <fieldset><legend>Optional file naming</legend>
   <label><input type="checkbox" checked={relocation.mode!=="off"} onChange={e=>update("relocation",e.target.checked?{mode:"rename_in_place",rule:"{artist} - {title}"}:{mode:"off"})}/>Rename files</label>
   {relocation.mode!=="off"?<><label>File naming mode<select value={relocation.mode} onChange={e=>{const mode=e.target.value as "rename_in_place"|"move"|"copy";update("relocation",mode==="rename_in_place"?{mode,rule:relocation.rule}:{mode,rule:relocation.rule,root_id:root?.id??""});}}><option value="rename_in_place">Rename in place</option><option value="move">Move and rename</option><option value="copy">Copy and rename</option></select></label><label>Filename pattern<input value={relocation.rule} onChange={e=>update("relocation",{...relocation,rule:e.target.value})}/></label>{relocation.mode!=="rename_in_place"?<div><p>Destination: {root?.display_path??relocation.root_id??"Choose a folder"}</p><button type="button" onClick={onChooseRoot}>Choose naming folder</button></div>:<p>Files stay in their current folder.</p>}<small>Moves and renames that need an unqualified source-library reference repair are blocked. Copies preserve the original reference.</small></>:null}
  </fieldset>
 </div>;
}
