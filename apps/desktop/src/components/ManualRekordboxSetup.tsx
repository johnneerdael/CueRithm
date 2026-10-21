import {useEffect,useRef,useState} from "react";
import {open} from "@tauri-apps/plugin-dialog";
import {FolderOpen,Save} from "lucide-react";
import {errorMessage} from "../api";
import type {RekordboxBridge,RekordboxManualTarget} from "../rekordbox-sync-types";
import type {SyncTargetConfiguration,TargetEnvironment} from "../sync-target-types";
import SyncTargetSetup from "./SyncTargetSetup";

const fields=[['executable','rekordbox executable'],['database','rekordbox database'],['options_file','rekordbox Agent options'],['asset_root','rekordbox asset folder']] as const;
async function folder(){const value=await open({directory:true,multiple:false});return typeof value==="string"?value:null;}
export default function ManualRekordboxSetup({bridge,onSaved,disabled}:{bridge:RekordboxBridge;onSaved:(value:SyncTargetConfiguration)=>void;disabled?:boolean}){
 const [name,setName]=useState(""),[paths,setPaths]=useState({executable:"",database:"",options_file:"",asset_root:""}),[version,setVersion]=useState("");
 const [environment,setEnvironment]=useState<TargetEnvironment>({kind:"native"}),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null);
 const alive=useRef(true),signature=JSON.stringify({name,paths,version,environment}),current=useRef(signature);current.current=signature;
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 const pick=async(key:keyof typeof paths)=>{const captured=signature;setError(null);try{const value=await open({directory:key==="asset_root",multiple:false});if(typeof value==="string"&&alive.current&&current.current===captured)setPaths(previous=>({...previous,[key]:value}));}catch(e){if(alive.current)setError(errorMessage(e));}};
 const save=async()=>{if(busy)return;setBusy(true);setError(null);const captured=signature;try{const request:RekordboxManualTarget={...paths,version_hint:version.trim()||null,environment};const target=await bridge.configure(request);const saved=await bridge.saveSyncTargetConfiguration({id:null,expected_revision:null,name:name.trim(),target:{kind:"rekordbox",target}});if(alive.current&&current.current===captured)onSaved(saved);}catch(e){if(alive.current)setError(errorMessage(e));}finally{if(alive.current)setBusy(false);}};
 return <details className="sync-manual-setup"><summary>Set up another library</summary><fieldset disabled={disabled||busy}>
  <label>Library setup name<input aria-label="Library setup name" maxLength={100} value={name} onChange={e=>setName(e.target.value)}/></label>
  <SyncTargetSetup value={environment} onChange={setEnvironment} chooseFolder={folder}/>
  {fields.map(([key,label])=><label key={key}>{label}<div className="sync-path"><input aria-label={label} value={paths[key]} onChange={e=>setPaths(previous=>({...previous,[key]:e.target.value}))}/><button type="button" className="secondary" aria-label={`Choose ${label}`} onClick={()=>void pick(key)}><FolderOpen size={14}/></button></div></label>)}
  <label>Version hint (optional)<input aria-label="rekordbox version hint" placeholder="7.2.19.0342" value={version} onChange={e=>setVersion(e.target.value)}/></label>
  <p className="sync-note">Use a version hint only when the executable has no readable version. Supported database and asset formats are checked before writing.</p>
  <button type="button" className="secondary" disabled={!name.trim()||fields.some(([key])=>!paths[key].trim())} onClick={()=>void save()}><Save size={14}/>{busy?"Saving library setup…":"Save library setup"}</button>
  {error?<p role="alert" className="attention">{error}</p>:null}
 </fieldset></details>;
}
