import {useEffect,useRef,useState} from "react";
import type {Bridge} from "../types";
import type {ProviderCredentialInput,ProviderSetupStatus} from "../provider-setup-types";
import {inspectorOperation} from "../inspector-operations";

export function providerSetupSummary(status:ProviderSetupStatus){
 const credential=status.credential;
 if(credential?.vault!==undefined&&credential.vault!=="ready")return credential.vault==="uncertain"?"Credential change needs attention":"Credential store · "+credential.vault;
 if(status.auth_mode==="anonymous")return "Anonymous · "+(status.rate_limit_per_minute??25)+" requests/minute";
 if(status.id==="musicbrainz")return "Public metadata · no account required";
 if(credential?.verification==="invalid")return "Credential rejected";
 if(credential?.verification==="email_unverified")return "Verify your email before use";
 if(credential?.verification==="restricted")return "Account access restricted";
 if(credential?.verification==="quota_exhausted")return "Quota exhausted";
 if(credential?.verification==="rate_limited")return "Rate or quota limit reached";
 if(credential?.presence==="present")return credential.verification==="valid"?"Connection verified":"Saved · not tested";
 return status.can_use?"Public metadata · no account required":"Optional · setup required";
}

export default function ProviderSetup({bridge,name,status,onRefresh}:{bridge:Bridge;name:string;status:ProviderSetupStatus;onRefresh:()=>Promise<void>}){
 const [open,setOpen]=useState(false),[replacing,setReplacing]=useState(false),[secret,setSecret]=useState(""),[email,setEmail]=useState(""),[password,setPassword]=useState(""),[busy,setBusy]=useState<string|null>(null),[activeOperation,setActiveOperation]=useState<string|null>(null),[error,setError]=useState<string|null>(null);
 const alive=useRef(true),sequence=useRef(0),operation=useRef<string|null>(null),latest=useRef(status);latest.current=status;
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;sequence.current++;if(operation.current)void bridge.cancelMetadataOperation(operation.current).catch(()=>{});};},[bridge]);
 const clear=()=>{setSecret("");setEmail("");setPassword("");setReplacing(false);};
 const run=async(label:string,call:()=>Promise<unknown>)=>{
  const request=++sequence.current;operation.current=null;setActiveOperation(null);setBusy(label);setError(null);
  try{await call();if(alive.current&&request===sequence.current)await onRefresh();}
  catch{if(alive.current&&request===sequence.current){setError(label.startsWith("Testing")?"Connection test did not finish. Refresh setup status or test again.":"Credential action did not finish. Refresh setup status and try again.");await onRefresh().catch(()=>{});}}
  finally{if(alive.current&&request===sequence.current){operation.current=null;setActiveOperation(null);setBusy(null);}}
 };
 const hasDraft=!!(secret||email||password);
 const credential=status.credential,hasSaved=credential?.presence==="present",account=status.setup_kind==="email_password",canEnter=!!credential&&status.setup_kind!=="none";
 const leased=<T,>(call:(id:string)=>Promise<T>)=>{const request=sequence.current;return inspectorOperation(bridge,id=>{if(!alive.current||request!==sequence.current)throw Error("Provider operation superseded");operation.current=id;setActiveOperation(id);return call(id);});};
 const save=()=>{const current=latest.current.credential;if(!current)return;const input:ProviderCredentialInput=account?{kind:"email_password",email,password}:{kind:status.setup_kind==="personal_token"?"personal_token":"api_key",secret};clear();void run("Saving credential…",()=>bridge.saveProviderCredential(current.reference,input));};
 const remove=()=>{const provider=latest.current.credential?.reference.provider;if(!provider)return;if(operation.current)void bridge.cancelMetadataOperation(operation.current).catch(()=>{});clear();void run("Removing credential…",()=>bridge.removeProviderCredential(provider));};
 if(status.setup_kind==="none")return null;
 return <div className="provider-setup">
  <button className="back-link" aria-expanded={open} onClick={()=>{if(open)clear();setOpen(value=>!value);}}>{open?"Close "+name+" setup":(hasSaved?"Manage ":"Set up ")+name}</button>
  {open?<section aria-label={name+" setup"} className="provider-setup-details">
   <p className="setting-note">Keys and accounts are saved to your system credential store. Save settings applies provider choices. Unsubmitted input is cleared when you leave.</p>
   {status.id==="sonovault"?<p className="setting-note"><a href="https://sonovault.now/signup" target="_blank" rel="noopener noreferrer">Get a free API key</a> · Verify your email before testing. Test connection uses one metadata credit; it never uploads audio or uses recognition.</p>:status.id==="discogs"?<p className="setting-note">Public search and lookups work without an account at 25 requests/minute. Optional verified authentication allows up to 60.</p>:null}
   <p role="status" className="provider-setup-message">{providerSetupSummary(status)}</p>
   {status.reason?<p className="provider-setup-message">{status.reason.message}</p>:null}
   {status.retry_at_ms!==null?<p className="setting-note">Next request after {new Date(status.retry_at_ms).toLocaleString()}. Requests remain paced by CueRithm.</p>:null}
   {status.quota?<p className="setting-note">{status.quota.remaining===null?"Remaining allowance unavailable":status.quota.remaining+" requests remaining"}{status.quota.resets_at_ms===null?" · reset time unavailable":" · resets "+new Date(status.quota.resets_at_ms).toLocaleString()}</p>:null}
   {canEnter&&(!hasSaved||replacing||hasDraft)?<div className="provider-secret-form">
    {account?<><label>{name} email<input type="email" aria-label={name+" email"} autoComplete="off" value={email} disabled={!!busy} onChange={e=>setEmail(e.target.value)}/></label><label>{name} password<input type="password" aria-label={name+" password"} autoComplete="off" value={password} disabled={!!busy} onChange={e=>setPassword(e.target.value)}/></label></>:<label>{name} {status.setup_kind==="personal_token"?"personal token":"API key"}<input type="password" aria-label={name+(status.setup_kind==="personal_token"?" personal token":" API key")} autoComplete="off" autoCapitalize="none" spellCheck={false} maxLength={4096} value={secret} disabled={!!busy} onChange={e=>setSecret(e.target.value)}/></label>}
    <button className="secondary" disabled={!!busy||(account?!email.trim()||!password:!secret)} onClick={save}>Save {name} {account?"account":status.setup_kind==="personal_token"?"token":"key"}</button>
    {replacing?<button className="back-link" disabled={!!busy} onClick={clear}>Cancel replacement</button>:null}
   </div>:hasSaved?<button className="secondary" disabled={!!busy} onClick={()=>{clear();setReplacing(true);}}>Replace {name} {account?"account":"key"}</button>:null}
   {credential&&credential.presence!=="missing"?<div className="artwork-actions"><button className="secondary" disabled={!!busy||!hasSaved||credential?.vault!=="ready"||status.can_test===false} onClick={()=>void run("Testing saved connection…",()=>leased(id=>bridge.testProviderCredential(id,credential!.reference)))}>Test saved {name} connection</button><button className="back-link" onClick={remove}>Remove {name} {account?"account":"key"}</button></div>:null}
  </section>:null}
  {busy?<p role="status" className="provider-setup-message">{busy}{activeOperation?<button className="back-link" onClick={()=>void bridge.cancelMetadataOperation(activeOperation).catch(()=>{})}>Cancel provider operation</button>:null}</p>:null}
  {error?<p role="alert" className="provider-setup-message">{error}</p>:null}
 </div>;
}
