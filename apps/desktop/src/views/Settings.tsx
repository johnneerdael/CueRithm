import {useEffect,useState} from "react";
import {FolderPlus,FolderOpen,X,Save,ChevronLeft,Check} from "lucide-react";
import type {Bridge,Snapshot,Settings as SettingsType,EngineSettings} from "../types";
import {errorMessage} from "../api";
export default function Settings({snapshot,bridge,onSaved,onBack}:{snapshot:Snapshot;bridge:Bridge;onSaved:()=>void;onBack:()=>void}){
  const [draft,setDraft]=useState<SettingsType>(snapshot.settings);
  const [busy,setBusy]=useState(false);const [error,setError]=useState<string|null>(null);const [saved,setSaved]=useState(false);
  useEffect(()=>{setDraft(snapshot.settings);},[snapshot.revision]);
  const engine=(patch:Partial<EngineSettings>)=>{setSaved(false);setDraft(d=>({...d,engine:{...d.engine,...patch}}));};
  return <section className="settings-view">
    <div className="view-title"><div><button className="back-link" onClick={onBack}><ChevronLeft size={15}/>Library</button><h1>Settings</h1><p>Configure preparation once. Incoming tracks take it from here.</p></div>
    <button className="primary" disabled={busy} onClick={async()=>{setBusy(true);setError(null);try{await bridge.saveSettings(draft);setSaved(true);onSaved();}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}}>{saved?<Check size={16}/>:<Save size={16}/>} {busy?"Saving…":saved?"Settings saved":"Save settings"}</button></div>
    {error?<div role="alert" className="error-banner">{error}</div>:null}
    <div className="settings-body">
      <section className="setting-section"><div className="setting-heading"><span className="section-number">01</span><div><h2>Incoming</h2><p>Files added to these folders are prepared automatically.</p></div></div>
        <div className="folder-list">{draft.watch_folders.map(path=><div className="folder-line" key={path}><FolderOpen size={17}/><span title={path}>{path}</span><button aria-label={"Remove watch folder "+path} onClick={()=>setDraft(d=>({...d,watch_folders:d.watch_folders.filter(p=>p!==path)}))}><X size={15}/></button></div>)}</div>
        <button className="secondary" onClick={async()=>{try{const path=await bridge.chooseFolder();if(path&&!draft.watch_folders.includes(path))setDraft(d=>({...d,watch_folders:[...d.watch_folders,path]}));}catch(e){setError(errorMessage(e));}}}><FolderPlus size={16}/>Add watch folder</button>
        <p className="setting-note">Completed tracks move into Tracks automatically. Original audio files stay in place.</p>
      </section>
      <section className="setting-section"><div className="setting-heading"><span className="section-number">02</span><div><h2>Analysis</h2><p>Key, BPM, beatgrid, one track energy rating and structural cues.</p></div></div>
        <p className="setting-note">All analysis engines are included and work offline. No model setup is required.</p>
      </section>
      <section className="setting-section"><div className="setting-heading"><span className="section-number">03</span><div><h2>Cue generation</h2><p>Validated structural cues, applied after full analysis.</p></div></div>
        <div className="setting-row"><div><strong>Existing cue points</strong><p>Replace after successful analysis. Failed preparation keeps the previous cues.</p></div><span className="fixed-setting"><Check size={14}/>Replace</span></div>
        <label className="setting-row"><div><strong>Minimum breakdown length</strong><p>Measured in beats.</p></div><input type="number" aria-label="Minimum breakdown length" min={1} max={4096} value={draft.engine.lexicon_config?.min_cue_length_beats??64} onChange={e=>engine({lexicon_config:{...draft.engine.lexicon_config!,min_cue_length_beats:Number(e.target.value)}})}/></label>
        <label className="setting-row"><div><strong>Drop at start</strong></div><select aria-label="Drop at start" value={draft.engine.lexicon_config?.drop_at_start??"Never"} onChange={e=>engine({lexicon_config:{...draft.engine.lexicon_config!,drop_at_start:e.target.value as "Never"|"Auto"}})}><option value="Never">Never</option><option value="Auto">Automatic</option></select></label>
        <label className="setting-row"><div><strong>Emergency loop</strong><p>Include a loop when the track has a suitable section.</p></div><input type="checkbox" checked={draft.engine.lexicon_config?.add_emergency_loop??false} onChange={e=>engine({lexicon_config:{...draft.engine.lexicon_config!,add_emergency_loop:e.target.checked}})}/></label>
      </section>
      <section className="setting-section future-settings"><div className="setting-heading"><span className="section-number">04</span><div><h2>Tagging & cleanup</h2><p>Beatport, Traxsource and cleanup settings will live here.</p></div></div><p className="setting-note">Not available in this build. Enabled settings from a later version will keep affected tracks in Incoming.</p></section>
    </div>
  </section>;
}
