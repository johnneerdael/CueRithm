import {useEffect,useRef,useState} from "react";
import {FolderOpen} from "lucide-react";
import type {OrganizationBridge,OrganizationSettings as Config,OrganizationRoot,OrganizationExample} from "../organization-types";
import {errorMessage} from "../api";
export default function OrganizationSettings({bridge,config,onChange,onValidation}:{bridge:OrganizationBridge;config:Config;onChange:(config:Config)=>void;onValidation?:(error:string|null)=>void}){
 const [roots,setRoots]=useState<OrganizationRoot[]>([]),[example,setExample]=useState<OrganizationExample|null>(null),[error,setError]=useState<string|null>(null),[checking,setChecking]=useState(false),[choosing,setChoosing]=useState(false);
 const input=useRef<HTMLInputElement>(null);
 useEffect(()=>{let alive=true;void bridge.organizationRoots().then(v=>{if(alive)setRoots(v);}).catch(e=>{if(alive)setError(errorMessage(e));});return()=>{alive=false;};},[bridge]);
 useEffect(()=>{
  let alive=true;setExample(null);setError(null);
  if(config.mode==="leave_in_place"){setChecking(false);onValidation?.(null);return;}
  setChecking(true);onValidation?.("Checking file organization rules…");
  void bridge.organizationExample(config).then(next=>{if(alive){setExample(next);onValidation?.(next.valid?null:next.error??"Review file organization rules.");}}).catch(e=>{if(alive){const message=errorMessage(e);setError(message);onValidation?.(message);}}).finally(()=>{if(alive)setChecking(false);});
  return()=>{alive=false;};
 },[bridge,config,onValidation]);
 const update=(patch:Partial<Config>)=>onChange({...config,...patch});
 const insert=(token:string)=>{const node=input.current,start=node?.selectionStart??config.pattern.length,end=node?.selectionEnd??start;update({pattern:config.pattern.slice(0,start)+"{"+token+"}"+config.pattern.slice(end)});requestAnimationFrame(()=>{node?.focus();node?.setSelectionRange(start+token.length+2,start+token.length+2);});};
 const destination=roots.find(root=>root.id===config.destination_root_id);
 return <section className="setting-section organization-settings"><div className="setting-heading"><div><h2>File organization</h2><p>Apply naming rules after tags, cleanup and analysis are ready.</p></div></div>
  <label className="setting-row"><div><strong>When preparation finishes</strong><p>Optional. Leave files in place is the default.</p></div><select aria-label="Organization mode" value={config.mode} onChange={e=>update({mode:e.target.value as Config["mode"]})}><option value="leave_in_place">Leave files in place</option><option value="rename_in_place">Rename in place</option><option value="move">Move to folder</option><option value="copy">Copy to folder</option></select></label>
  {config.mode!=="leave_in_place"?<>
   {config.mode==="move"||config.mode==="copy"?<div className="setting-row organization-destination"><div><strong>Destination folder</strong><p className="organization-path">{destination?.display_path??(config.destination_root_id?"Saved folder is unavailable. Choose it again.":"Choose where completed files belong.")}</p></div><button type="button" className="secondary" disabled={choosing} onClick={async()=>{setChoosing(true);try{const root=await bridge.chooseOrganizationRoot();if(root){setRoots(old=>[...old.filter(r=>r.id!==root.id),root]);update({destination_root_id:root.id});}}catch(e){setError(errorMessage(e));}finally{setChoosing(false);}}}><FolderOpen size={15}/>{choosing?"Choosing…":"Choose destination"}</button></div>:null}
   <label className="organization-pattern"><strong>Filename pattern</strong><small>Use accepted metadata. The file extension is kept.</small><input ref={input} aria-label="Filename pattern" value={config.pattern} onChange={e=>update({pattern:e.target.value})}/></label>
   <div className="organization-tokens" aria-label="Insert naming token">{["artist","title","version","label","year","bpm","key","energy"].map(token=><button key={token} type="button" className="secondary" onClick={()=>insert(token)}>{token==="version"?"Mix / version":token.charAt(0).toUpperCase()+token.slice(1)}</button>)}</div>
   <details><summary>Advanced pattern syntax</summary><p className="setting-note">Use braces for metadata, such as <code>{"{title}"}</code>. Put optional parts in square brackets, such as <code>{"[ ({version})]"}</code>. Folder separators are available for move and copy rules.</p></details>
   <label className="setting-row"><div><strong>Missing values</strong><p>Required metadata must be present. Optional parts use square brackets.</p></div><select aria-label="Missing organization values" value={config.missing} onChange={e=>update({missing:e.target.value as Config["missing"]})}><option value="block">Needs attention</option><option value="omit_optional">Omit optional part</option><option value="skip_optional" disabled={!config.organization_optional}>Keep original name (optional organization)</option></select></label>
   <label className="setting-row"><div><strong>Organization is optional</strong><p>Allow the configured skip policy to finish preparation without organizing a track.</p></div><input aria-label="Organization is optional" type="checkbox" checked={config.organization_optional} onChange={e=>update({organization_optional:e.target.checked,...(!e.target.checked?{collision:"block",missing:config.missing==="skip_optional"?"block":config.missing}:{})})}/></label>
   <label className="setting-row"><div><strong>If a filename already exists</strong><p>Existing files are never overwritten.</p></div><select aria-label="Organization collision policy" value={config.collision} onChange={e=>update({collision:e.target.value as Config["collision"]})}><option value="block">Needs attention</option><option value="skip" disabled={!config.organization_optional}>Skip track (optional organization)</option></select></label>
   {checking?<p role="status">Checking naming rules…</p>:null}
   {example?<div className="organization-examples" aria-label="Example filenames"><strong>Example filenames</strong><small>Invented metadata · not an apply plan</small>{example.error?<p role="alert" className="attention">{example.error}</p>:null}{example.examples.map((row,index)=><div key={index}><span className="organization-path muted">{row.original_name}</span><span className="organization-path">→ {row.result??row.reason??"Needs attention"}</span></div>)}</div>:null}
   <p className="setting-note">Every Incoming track is checked against its final metadata. Known linked DJ libraries must be repaired before a move finishes; unknown external references cannot be checked.</p>
  </>:<p className="setting-note">Original audio files stay in place. Enable a mode to organize future Incoming tracks automatically.</p>}
  {error?<p role="alert" className="error-banner">{error}</p>:null}
 </section>;
}
