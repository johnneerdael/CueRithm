import {useEffect,useId,useState} from "react";
import {ArrowDown,ArrowUp,Plus,X} from "lucide-react";
import {artistPartMessage,validArtistPart,smartFixTextError,rustTrim} from "../cleanup-validation";
export {cleanupValidationError} from "../cleanup-validation";
import type {CleanupConfig,SmartFix,MetadataField,ContributorRole,CanonicalMapping,CleanupCatalog,CleanupCatalogValue,GenreRules,ArtistRules,CommonTextPattern} from "../types";

export const emptyCleanup=():CleanupConfig=>({fixes:[],genres:{enabled:false,mappings:[],protected_values:[],conversion:null},artists:{enabled:false,roles:["artist"],mappings:[],protected_values:[]}});
const fields:MetadataField[]=["title","artist","album","album_artist","genre","comment","grouping","label","remixer","mix","producer","composer","lyricist"];
const roles:ContributorRole[]=["artist","remixer","producer","composer","lyricist"];
const templates:{name:string;rule:SmartFix}[]=[
 {name:"Casing",rule:{kind:"title_case",field:"title",uppercase:true,lowercase:true}},
 {name:"Characters to spaces",rule:{kind:"replace_characters",characters:"_"}},
 {name:"Encoded characters",rule:{kind:"decode_entities"}},
 {name:"Extract artist",rule:{kind:"extract_artist",separator:" - ",part:1}},
 {name:"Extract remixer",rule:{kind:"extract_remixer",word:"remix",remove_title:true}},
 {name:"Garbage",rule:{kind:"remove_garbage",field:"title"}},
 {name:"Number prefix",rule:{kind:"remove_number_prefix"}},
 {name:"URLs and email",rule:{kind:"remove_urls",field:"title",clear_field:false}},
 {name:"Mix parentheses",rule:{kind:"mix_parentheses"}},
 {name:"Common title text",rule:{kind:"remove_common_text",pattern:"OriginalMixFromTitle"}},
 {name:"Promotional text",rule:{kind:"remove_promotional_text",field:"title"}},
];
export const fixName=(rule:SmartFix)=>templates.find(t=>t.rule.kind===rule.kind)?.name??rule.kind;
export function LineValues({label,values,onChange}:{label:string;values:string[];onChange:(values:string[])=>void}){
 const [text,setText]=useState(values.join("\n"));const signature=JSON.stringify(values);
 useEffect(()=>setText(values.join("\n")),[signature]);
 return <label className="cleanup-lines">{label}<textarea aria-label={label} rows={2} value={text} onChange={e=>{setText(e.target.value);onChange([...new Set(e.target.value.split("\n").filter(v=>rustTrim(v)))]);}}/></label>;
}
function FixTextInput({label,value,error,onChange}:{label:string;value:string;error:string|null;onChange:(value:string)=>void}){
 const description=useId();
 return <label>{label}<input aria-label={label} aria-invalid={error?true:undefined} aria-describedby={description} value={value} onChange={e=>onChange(e.target.value)}/><small id={description} role={error?"alert":undefined}>{Array.from(value).length}/64 characters{error?`. ${error}`:""}</small></label>;
}
function ArtistResultInput({part,onChange}:{part:number;onChange:(part:number)=>void}){
 const description=useId(),invalid=!validArtistPart(part,true);
 return <label>Result number<input aria-label="Result number" type="number" min={1} max={256} step={1} aria-invalid={invalid?true:undefined} aria-describedby={description} value={Number.isFinite(part)?part:""} onChange={e=>onChange(e.target.valueAsNumber)}/><small id={description} role={invalid?"alert":undefined}>{invalid?artistPartMessage:"Whole number from 1 to 256."}</small></label>;
}
function FixOptions({rule,onChange}:{rule:SmartFix;onChange:(rule:SmartFix)=>void}){
 return <div className="cleanup-options">
  {"field" in rule?<label>Field<select aria-label="Field" value={rule.field} onChange={e=>onChange({...rule,field:e.target.value as MetadataField})}>{fields.map(f=><option key={f} value={f}>{f.replaceAll("_"," ")}</option>)}</select></label>:null}
  {rule.kind==="title_case"?<><label><input type="checkbox" checked={rule.uppercase} onChange={e=>onChange({...rule,uppercase:e.target.checked})}/>Uppercase values</label><label><input type="checkbox" checked={rule.lowercase} onChange={e=>onChange({...rule,lowercase:e.target.checked})}/>Lowercase values</label></>:null}
  {rule.kind==="replace_characters"?<FixTextInput label="Characters" value={rule.characters} error={smartFixTextError(rule)} onChange={characters=>onChange({...rule,characters})}/>:null}
  {rule.kind==="extract_artist"?<><FixTextInput label="Separator" value={rule.separator} error={smartFixTextError(rule)} onChange={separator=>onChange({...rule,separator})}/><ArtistResultInput part={rule.part} onChange={part=>onChange({...rule,part})}/></>:null}
  {rule.kind==="extract_remixer"?<><FixTextInput label="Remixer suffix" value={rule.word} error={smartFixTextError(rule)} onChange={word=>onChange({...rule,word})}/><label><input type="checkbox" checked={rule.remove_title} onChange={e=>onChange({...rule,remove_title:e.target.checked})}/>Remove remixer from title</label></>:null}
  {rule.kind==="remove_urls"?<label><input type="checkbox" checked={rule.clear_field} onChange={e=>onChange({...rule,clear_field:e.target.checked})}/>Clear entire matching field</label>:null}
  {rule.kind==="remove_common_text"?<label>Pattern<select aria-label="Pattern" value={rule.pattern} onChange={e=>onChange({...rule,pattern:e.target.value as CommonTextPattern})}><option value="OriginalMixFromTitle">Original Mix suffix</option><option value="CamelotKeyFromTitle">Camelot key</option><option value="CamelotKeyEnergyFromTitle">Camelot key and energy</option><option value="BpmFromTitle">BPM suffix</option></select></label>:null}
 </div>;
}
export function SmartFixEditor({config,onChange}:{config:CleanupConfig;onChange:(config:CleanupConfig)=>void}){
 const [newRule,setNewRule]=useState(templates[0].name);
 const update=(index:number,patch:Partial<CleanupConfig["fixes"][number]>)=>onChange({...config,fixes:config.fixes.map((f,i)=>i===index?{...f,...patch}:f)});
 const move=(index:number,by:number)=>{const fixes=[...config.fixes];[fixes[index],fixes[index+by]]=[fixes[index+by],fixes[index]];onChange({...config,fixes});};
 return <div className="cleanup-rule-list">
  <p className="cleanup-note">Rules run in this order, before genre and artist mappings. Only enabled rules are applied.</p>
  {config.fixes.map((fix,i)=><fieldset className="cleanup-rule" key={fix.id}><legend>Smart Fix: {fixName(fix.rule)}</legend>
   <div className="cleanup-rule-heading"><label><input type="checkbox" checked={fix.enabled} onChange={e=>update(i,{enabled:e.target.checked})}/>Enabled</label><span className="cleanup-rule-order">{i+1}</span><button aria-label="Move up" disabled={i===0} onClick={()=>move(i,-1)}><ArrowUp size={14}/></button><button aria-label="Move down" disabled={i===config.fixes.length-1} onClick={()=>move(i,1)}><ArrowDown size={14}/></button><button aria-label="Remove rule" onClick={()=>onChange({...config,fixes:config.fixes.filter((_,n)=>n!==i)})}><X size={14}/></button></div>
   <FixOptions rule={fix.rule} onChange={rule=>update(i,{rule})}/>
   <LineValues label="Ignored values, one per line" values={fix.ignored_values} onChange={ignored_values=>update(i,{ignored_values})}/>
  </fieldset>)}
  <div className="cleanup-add"><select aria-label="New Smart Fix" value={newRule} onChange={e=>setNewRule(e.target.value)}>{templates.map(t=><option key={t.name}>{t.name}</option>)}</select><button className="secondary" disabled={config.fixes.length>=64} onClick={()=>onChange({...config,fixes:[...config.fixes,{id:crypto.randomUUID(),enabled:true,ignored_values:[],rule:{...templates.find(t=>t.name===newRule)!.rule}}]})}><Plus size={14}/>Add Smart Fix</button></div>
 </div>;
}
function MappingEditor({mappings,onChange}:{mappings:CanonicalMapping[];onChange:(mappings:CanonicalMapping[])=>void}){
 return <div className="cleanup-mappings">{mappings.map((m,i)=><div className="cleanup-mapping" key={i}><input aria-label={"Source value "+(i+1)} placeholder="Exact source value" value={m.from} onChange={e=>onChange(mappings.map((v,n)=>n===i?{...v,from:e.target.value}:v))}/><span>→</span><input aria-label={"Canonical value "+(i+1)} placeholder="Canonical value" disabled={m.to===null} value={m.to??""} onChange={e=>onChange(mappings.map((v,n)=>n===i?{...v,to:e.target.value}:v))}/><label><input type="checkbox" checked={m.to===null} onChange={e=>onChange(mappings.map((v,n)=>n===i?{...v,to:e.target.checked?null:""}:v))}/>Remove</label><button aria-label={"Remove mapping "+(i+1)} onClick={()=>onChange(mappings.filter((_,n)=>n!==i))}><X size={14}/></button></div>)}<button className="secondary" onClick={()=>onChange([...mappings,{from:"",to:""}])}><Plus size={14}/>Add mapping</button></div>;
}
export function CanonicalEditor({kind,config,onChange,catalog}:{kind:"genres"|"artists";config:CleanupConfig;onChange:(c:CleanupConfig)=>void;catalog?:CleanupCatalog|null}){
 const [search,setSearch]=useState("");const [order,setOrder]=useState("value");
 const rules=config[kind];const update=(patch:Partial<GenreRules>|Partial<ArtistRules>)=>onChange({...config,[kind]:{...rules,...patch}});
 const entries:CleanupCatalogValue[]=catalog?.[kind]??[];
 const visible=entries.filter(v=>v.value.toLocaleLowerCase().includes(search.toLocaleLowerCase())).slice().sort((a,b)=>order==="count"?b.track_count-a.track_count||a.value.localeCompare(b.value):a.value.localeCompare(b.value));
 const replace=(value:string,to:string)=>update({mappings:[...rules.mappings.filter(m=>m.from!==value),...(to.length?[{from:value,to}]:[])]});
 const protect=(value:string,checked:boolean)=>update({protected_values:checked?[...new Set([...rules.protected_values,value])]:rules.protected_values.filter(v=>v!==value)});
 return <div className="canonical-editor">
  <label className="setting-row"><strong>Enable {kind==="genres"?"genre":"artist"} cleanup</strong><input type="checkbox" checked={rules.enabled} onChange={e=>update({enabled:e.target.checked})}/></label>
  {kind==="artists"?<div className="cleanup-options">{roles.map(role=><label key={role}><input type="checkbox" checked={config.artists.roles.includes(role)} onChange={e=>update({roles:e.target.checked?[...config.artists.roles,role]:config.artists.roles.filter(v=>v!==role)})}/>{role}</label>)}</div>:null}
  <p className="cleanup-note">Map exact values to the spelling you want. Protected values remain unchanged.</p>
  <MappingEditor mappings={rules.mappings} onChange={mappings=>update({mappings})}/>
  <LineValues label={"Protected "+kind+", one per line"} values={rules.protected_values} onChange={protected_values=>update({protected_values})}/>
  {kind==="genres"?<div className="cleanup-conversion"><label><input type="checkbox" checked={!!config.genres.conversion} onChange={e=>update({conversion:e.target.checked?{category:"Subgenres",genres:[],remove_genre:false}:null})}/>Convert genres into categorized tags</label>{config.genres.conversion?<><label>Category<input aria-label="Genre tag category" value={config.genres.conversion.category} onChange={e=>update({conversion:{...config.genres.conversion!,category:e.target.value}})}/></label><LineValues label="Genres to convert, one per line" values={config.genres.conversion.genres} onChange={genres=>update({conversion:{...config.genres.conversion!,genres}})}/><label><input type="checkbox" checked={config.genres.conversion.remove_genre} onChange={e=>update({conversion:{...config.genres.conversion!,remove_genre:e.target.checked}})}/>Remove converted values from Genre</label></>:null}</div>:null}
  {catalog?<><div className="cleanup-catalog-toolbar"><input type="search" aria-label={"Search "+kind} placeholder={"Search "+kind} value={search} onChange={e=>setSearch(e.target.value)}/><select aria-label={"Sort "+kind} value={order} onChange={e=>setOrder(e.target.value)}><option value="value">Name</option><option value="count">Track count</option></select></div>
  <div className="cleanup-catalog">{visible.map(entry=><div className="cleanup-catalog-row" key={entry.value}><div><strong>{entry.value}</strong><small>{entry.track_count} {entry.track_count===1?"track":"tracks"}</small>{kind==="artists"?<small>{Object.entries(catalog.artists.find(v=>v.value===entry.value)!.roles).map(([r,n])=>r+": "+n).join(" · ")}</small>:null}</div><input aria-label={"Replace "+entry.value+" with"} placeholder="Keep existing" value={rules.mappings.find(m=>m.from===entry.value)?.to??""} onChange={e=>replace(entry.value,e.target.value)}/><label><input type="checkbox" aria-label={"Protect "+entry.value} checked={rules.protected_values.includes(entry.value)} onChange={e=>protect(entry.value,e.target.checked)}/>Protect</label></div>)}</div>{!visible.length?<p className="cleanup-note">No matching {kind}.</p>:null}</>:null}
 </div>;
}
export default function CleanupSettings({config,enabled,onChange}:{config?:CleanupConfig|null;enabled:boolean;onChange:(c:CleanupConfig,enabled:boolean)=>void}){
 const value=config??emptyCleanup();
 return <section className="setting-section"><div className="setting-heading"><div><h2>Cleanup</h2><p>Shared rules for automatic Incoming and manual cleanup.</p></div></div>
  <label className="setting-row"><div><strong>Clean incoming tracks automatically</strong><p>Apply after catalog tags, before completed tracks enter your library.</p></div><input type="checkbox" aria-label="Clean incoming tracks automatically" checked={enabled} onChange={e=>onChange(value,e.target.checked)}/></label>
  <h3>Smart Fixes</h3><SmartFixEditor config={value} onChange={c=>onChange(c,enabled)}/>
  <h3>Genres</h3><CanonicalEditor kind="genres" config={value} onChange={c=>onChange(c,enabled)}/>
  <h3>Artists</h3><CanonicalEditor kind="artists" config={value} onChange={c=>onChange(c,enabled)}/>
 </section>;
}
