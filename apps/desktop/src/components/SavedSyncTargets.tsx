import {useEffect,useMemo,useRef,useState} from "react";
import {Save,Trash2} from "lucide-react";
import {errorMessage} from "../api";
import type {DjSoftware} from "../sync-types";
import type {SavedSyncTarget,SyncConfigurationBridge,SyncTargetConfiguration} from "../sync-target-types";
export interface SavedSyncTargetsProps {bridge:SyncConfigurationBridge;kind:SavedSyncTarget["kind"];software?:DjSoftware;value:SavedSyncTarget|null;onSelected:(value:SyncTargetConfiguration|null)=>void;disabled?:boolean;refreshKey?:number;selectedConfiguration?:SyncTargetConfiguration|null}
export default function SavedSyncTargets({bridge,kind,software,value,onSelected,disabled,refreshKey,selectedConfiguration}:SavedSyncTargetsProps){
 const [rows,setRows]=useState<SyncTargetConfiguration[]>([]),[selected,setSelected]=useState(""),[name,setName]=useState(""),[busy,setBusy]=useState(false),[loading,setLoading]=useState(true),[error,setError]=useState<string|null>(null);
 const scope=useMemo(()=>({}),[bridge,kind,software]),loadedScope=useRef<object|null>(null);
 const context=useMemo(()=>({}),[scope,refreshKey]),contextRef=useRef(context);contextRef.current=context;
 const alive=useRef(true),current=useRef(value),callback=useRef(onSelected);current.current=value;callback.current=onSelected;
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 useEffect(()=>{let active=true;setLoading(true);setRows([]);setError(null);if(loadedScope.current!==scope){loadedScope.current=scope;setSelected("");setName("");callback.current(null);}void bridge.syncTargetConfigurations().then(values=>{if(active)setRows(values.filter(row=>row.target.kind===kind&&(kind!=="dj_library"||!software||row.target.kind==="dj_library"&&row.target.target.software===software)));}).catch(e=>{if(active)setError(errorMessage(e));}).finally(()=>{if(active)setLoading(false);});return()=>{active=false;};},[bridge,kind,software,refreshKey,context,scope]);
 useEffect(()=>{if(selectedConfiguration!==undefined){setSelected(selectedConfiguration?.id??"");setName(selectedConfiguration?.name??"");}},[selectedConfiguration]);
 const save=async()=>{if(!value||!name.trim()||busy||loading)return;const captured=value,capturedContext=context,record=rows.find(row=>row.id===selected);setBusy(true);setError(null);try{const saved=await bridge.saveSyncTargetConfiguration({id:record?.id??null,expected_revision:record?.revision??null,name:name.trim(),target:captured});if(alive.current&&contextRef.current===capturedContext){setRows(previous=>[...previous.filter(row=>row.id!==saved.id),saved]);if(current.current===captured){setSelected(saved.id);setName(saved.name);callback.current(saved);}}}catch(e){if(alive.current&&contextRef.current===capturedContext)setError(errorMessage(e));}finally{if(alive.current)setBusy(false);}};
 const remove=async()=>{const record=rows.find(row=>row.id===selected);if(!record||busy||loading)return;const capturedContext=context;setBusy(true);setError(null);try{await bridge.deleteSyncTargetConfiguration(record.id,record.revision);if(alive.current&&contextRef.current===capturedContext){setRows(previous=>previous.filter(row=>row.id!==record.id));setSelected("");setName("");callback.current(null);}}catch(e){if(alive.current&&contextRef.current===capturedContext)setError(errorMessage(e));}finally{if(alive.current)setBusy(false);}};
 return <fieldset className="sync-saved-setups" disabled={disabled||busy||loading}>
  <label>Saved library setup<select aria-label="Saved library setup" value={selected} onChange={e=>{const row=rows.find(row=>row.id===e.target.value);setSelected(row?.id??"");setName(row?.name??"");setError(null);callback.current(row??null);}}><option value="">Unsaved setup</option>{rows.map(row=><option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
  <label>Setup name<input aria-label="Setup name" maxLength={100} value={name} onChange={e=>setName(e.target.value)}/></label>
  <div className="sync-actions"><button type="button" className="secondary" disabled={!value||!name.trim()} onClick={()=>void save()}><Save size={14}/>Save setup</button><button type="button" className="secondary" disabled={!selected} onClick={()=>void remove()}><Trash2 size={14}/>Delete saved setup</button></div>
  <p className="sync-note">Saving keeps this setup for later. Sync starts only after you preview and apply changes.</p>
  {error?<p role="alert" className="attention">{error}</p>:null}
 </fieldset>;
}
