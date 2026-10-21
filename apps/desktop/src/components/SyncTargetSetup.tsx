import {useEffect,useRef,useState} from "react";
import {FolderOpen,Plus,Trash2} from "lucide-react";
import {errorMessage} from "../api";
import type {TargetEnvironment} from "../sync-target-types";
export interface SyncTargetSetupProps {value:TargetEnvironment;onChange:(value:TargetEnvironment)=>void;chooseFolder:()=>Promise<string|null>;disabled?:boolean}
export default function SyncTargetSetup({value,onChange,chooseFolder,disabled}:SyncTargetSetupProps){
 const [error,setError]=useState<string|null>(null),[picking,setPicking]=useState(false);
 const current=useRef(value),alive=useRef(true);current.current=value;
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 const pick=async(change:(path:string)=>TargetEnvironment)=>{const captured=value;setPicking(true);setError(null);try{const path=await chooseFolder();if(path&&alive.current&&current.current===captured)onChange(change(path));}catch(e){if(alive.current&&current.current===captured)setError(errorMessage(e));}finally{if(alive.current)setPicking(false);}};
 return <fieldset className="sync-target-setup" disabled={disabled}>
  <label>Application environment<select aria-label="Application environment" value={value.kind} onChange={e=>{setError(null);onChange(e.target.value==="native"?{kind:"native"}:{kind:"windows_compatibility",prefix:"",installation:null,mappings:[]});}}><option value="native">Native installation</option><option value="windows_compatibility">Windows through Wine / Proton</option></select></label>
  {value.kind==="windows_compatibility"?<>
   <label>Wine or Proton prefix<div className="sync-path"><input aria-label="Wine or Proton prefix" value={value.prefix} onChange={e=>onChange({...value,prefix:e.target.value})}/><button type="button" className="secondary" aria-label="Choose Wine or Proton prefix" disabled={picking} onClick={()=>void pick(prefix=>({...value,prefix}))}><FolderOpen size={14}/></button></div></label>
   <label>Windows installation folder<div className="sync-path"><input aria-label="Windows installation folder" value={value.installation??""} onChange={e=>onChange({...value,installation:e.target.value||null})}/><button type="button" className="secondary" aria-label="Choose Windows installation folder" disabled={picking} onClick={()=>void pick(installation=>({...value,installation}))}><FolderOpen size={14}/></button></div></label>
   <p className="sync-note">Map the Windows drives or shares used by this library to their folders on this computer. Preview reports paths that cannot be resolved.</p>
   {value.mappings.map((mapping,index)=><div className="sync-drive-mapping" key={index}>
    <label>Windows root {index+1}<input aria-label={`Windows root ${index+1}`} placeholder="D:\Music" value={mapping.windows_root} onChange={e=>onChange({...value,mappings:value.mappings.map((m,i)=>i===index?{...m,windows_root:e.target.value}:m)})}/></label>
    <label>Host folder {index+1}<div className="sync-path"><input aria-label={`Host folder ${index+1}`} placeholder="/mnt/music" value={mapping.host_root} onChange={e=>onChange({...value,mappings:value.mappings.map((m,i)=>i===index?{...m,host_root:e.target.value}:m)})}/><button type="button" className="secondary" aria-label={`Choose host folder ${index+1}`} disabled={picking} onClick={()=>void pick(host_root=>({...value,mappings:value.mappings.map((m,i)=>i===index?{...m,host_root}:m)}))}><FolderOpen size={14}/></button></div></label>
    <button type="button" className="secondary" aria-label={`Remove drive mapping ${index+1}`} onClick={()=>onChange({...value,mappings:value.mappings.filter((_,i)=>i!==index)})}><Trash2 size={14}/></button>
   </div>)}
   <button type="button" className="secondary" onClick={()=>onChange({...value,mappings:[...value.mappings,{windows_root:"",host_root:""}]})}><Plus size={14}/>Add drive mapping</button>
  </>:null}
  {error?<p role="alert" className="attention">{error}</p>:null}
 </fieldset>;
}
