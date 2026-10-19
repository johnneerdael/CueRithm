import {useCallback,useEffect,useRef,useState} from "react";
import {Save,RefreshCw,LoaderCircle} from "lucide-react";
import type {Bridge} from "../types";
import type {PlaylistPreferenceSnapshot,PlaylistPreferences as Preferences} from "../playlist-types";
import {errorMessage} from "../api";
export default function PlaylistPreferences({bridge,onSaved}:{bridge:Bridge;onSaved:()=>void}){
 const [stored,setStored]=useState<PlaylistPreferenceSnapshot|null>(null),[draft,setDraft]=useState<Preferences>({time_zone:null,key_mixing_mode:"harmonic"});
 const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[saved,setSaved]=useState(false);
 const alive=useRef(true),request=useRef(0),writing=useRef(false);
 const load=useCallback(async()=>{const generation=++request.current;setBusy(true);setError(null);setSaved(false);try{const next=await bridge.getPlaylistPreferences();if(alive.current&&generation===request.current){setStored(next);setDraft(next.preferences);}}catch(e){if(alive.current&&generation===request.current)setError(errorMessage(e));}finally{if(alive.current&&generation===request.current)setBusy(false);}},[bridge]);
 useEffect(()=>{alive.current=true;void load();return()=>{alive.current=false;request.current++;};},[load]);
 const dirty=stored&&JSON.stringify(draft)!==JSON.stringify(stored.preferences);
 const update=(next:Preferences)=>{setDraft(next);setSaved(false);};
 const save=async()=>{if(!stored||!dirty||writing.current)return;writing.current=true;setBusy(true);setError(null);setSaved(false);try{const next=await bridge.savePlaylistPreferences({...draft,time_zone:draft.time_zone?.trim()||null},stored.revision);if(alive.current){setStored(next);setDraft(next.preferences);setSaved(true);onSaved();}}catch(e){if(alive.current)setError(errorMessage(e));}finally{writing.current=false;if(alive.current)setBusy(false);}};
 return <section className="setting-section" aria-label="Library preferences"><div className="setting-heading"><div><h2>Library preferences</h2><p>Calendar and key-matching rules for smart playlists.</p></div></div>
  {error?<div role="alert" className="error-banner">{error}</div>:null}
  {!stored&&busy?<p role="status"><LoaderCircle className="spin" size={14}/>Loading library preferences…</p>:null}
  <fieldset className="settings-fields" disabled={!stored||busy}>
   <label className="setting-row"><div><strong>Time zone</strong><p>Use an IANA name, or leave blank to follow the system time zone. Calendar rules use this zone.</p></div><input aria-label="Library time zone" placeholder="System time zone" value={draft.time_zone??""} maxLength={128} onChange={e=>update({...draft,time_zone:e.target.value||null})}/></label>
   <label className="setting-row"><div><strong>Key compatibility</strong><p>Used by smart playlist key-matching rules.</p></div><select aria-label="Key compatibility mode" value={draft.key_mixing_mode} onChange={e=>update({...draft,key_mixing_mode:e.target.value as Preferences["key_mixing_mode"]})}><option value="harmonic">Harmonic</option><option value="fuzzy">Fuzzy</option></select></label>
  </fieldset>
  <div className="playlist-preferences-actions"><button className="primary" disabled={!dirty||busy} onClick={()=>void save()}><Save size={14}/>{busy&&stored?"Saving…":"Save library preferences"}</button>{error?<button className="secondary" disabled={busy} onClick={()=>void load()}><RefreshCw size={14}/>Reload library preferences</button>:null}{saved?<span role="status">Library preferences saved.</span>:null}</div>
 </section>;
}
