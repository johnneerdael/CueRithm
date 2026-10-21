export interface RecognitionConfig {
  enabled:boolean;
  provider:"acoustid";
  fingerprint_consent:boolean;
  audio_upload_consent:boolean;
  max_attempts_per_track:1;
}
export const defaultRecognition:RecognitionConfig={enabled:false,provider:"acoustid",fingerprint_consent:false,audio_upload_consent:false,max_attempts_per_track:1};
export default function RecognitionSettings({config,configured,onChange}:{config?:RecognitionConfig;configured:boolean;onChange:(value:RecognitionConfig)=>void}){
  const value=config??defaultRecognition;
  return <details className="tagging-group"><summary>Identify unresolved recordings</summary>
    <p className="setting-note">Chromaprint creates an audio fingerprint on your computer. AcoustID looks it up, then MusicBrainz supplies candidates for match review. Key, BPM, energy and cues keep their measured analysis.</p>
    {!configured?<p className="setting-note" role="status">Fingerprint lookup is unavailable in this build because its AcoustID application identity is missing.</p>:null}
    <label className="setting-row"><div><strong>Allow fingerprint lookup</strong><p>Send a compressed fingerprint and track duration to AcoustID when catalog matching cannot resolve a readable track. No audio file is uploaded. This consent stays local and is excluded from exported presets.</p></div><input aria-label="Allow fingerprint lookup" type="checkbox" disabled={!configured&&!value.fingerprint_consent} checked={value.fingerprint_consent} onChange={e=>onChange({...value,fingerprint_consent:e.target.checked,enabled:e.target.checked&&value.enabled})}/></label>
    <label className="setting-row"><div><strong>Identify unresolved tracks</strong><p>One identification lookup per preparation attempt. Results remain in Incoming for edition review before tagging and final file organization.</p></div><input aria-label="Identify unresolved tracks" type="checkbox" disabled={(!configured||!value.fingerprint_consent)&&!value.enabled} checked={value.enabled} onChange={e=>onChange({...value,enabled:e.target.checked})}/></label>
  </details>;
}
