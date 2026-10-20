import {Plus,X} from "lucide-react";
import type {AutomaticPlaylistSettings} from "../playlist-types";
export const automaticDefaults:AutomaticPlaylistSettings={artists:false,bpm:false,bpm_ranges:[],release_dates:false,genres:false,keys:false};
export function automaticError(value:AutomaticPlaylistSettings):string|null {
 if(value.bpm_ranges.length>10)return "Configure at most 10 BPM ranges.";
 for(const [index,range] of value.bpm_ranges.entries()){
  if(!Number.isFinite(range.start)||!Number.isFinite(range.end)||range.start<=0||range.end>999||range.start>range.end)return `Range ${index+1}: enter a minimum and maximum above 0 and up to 999 BPM, with minimum no greater than maximum.`;
  if(value.bpm_ranges.slice(0,index).some(r=>r.start===range.start&&r.end===range.end))return "Each BPM range must be different.";
 }
 return null;
}
export default function AutomaticPlaylistOptions({value,onChange}:{value:AutomaticPlaylistSettings;onChange:(value:AutomaticPlaylistSettings)=>void}){
 const toggle=(key:"artists"|"bpm"|"release_dates"|"genres"|"keys",checked:boolean)=>onChange({...value,[key]:checked});
 const range=(index:number,field:"start"|"end",text:string)=>onChange({...value,bpm_ranges:value.bpm_ranges.map((r,i)=>i===index?{...r,[field]:text===""?NaN:Number(text)}:r)});
 return <>
  <p className="setting-note">Choose which lists to create from prepared tracks. New artists, genres and release months are added automatically after you save.</p>
  <label className="setting-row"><div><strong>By Artist</strong><p>One list per individual artist or remixer, including collaboration credits.</p></div><input aria-label="By Artist" type="checkbox" checked={value.artists} onChange={e=>toggle("artists",e.target.checked)}/></label>
  <label className="setting-row"><div><strong>By BPM</strong><p>Up to 10 inclusive ranges. Overlapping ranges can share tracks.</p></div><input aria-label="By BPM" type="checkbox" checked={value.bpm} onChange={e=>toggle("bpm",e.target.checked)}/></label>
  {value.bpm?<div className="automatic-bpm-ranges">
   {value.bpm_ranges.map((r,i)=><div className="automatic-bpm-range" key={i}><span>Range {i+1}</span><label>From<input aria-label={`Range ${i+1} minimum BPM`} type="number" min="0.01" max="999" step="0.01" value={Number.isFinite(r.start)?r.start:""} onChange={e=>range(i,"start",e.target.value)}/></label><label>To<input aria-label={`Range ${i+1} maximum BPM`} type="number" min="0.01" max="999" step="0.01" value={Number.isFinite(r.end)?r.end:""} onChange={e=>range(i,"end",e.target.value)}/></label><button className="icon-button" aria-label={`Remove BPM range ${i+1}`} onClick={()=>onChange({...value,bpm_ranges:value.bpm_ranges.filter((_,n)=>n!==i)})}><X size={15}/></button></div>)}
   <button className="secondary" disabled={value.bpm_ranges.length>=10} onClick={()=>onChange({...value,bpm_ranges:[...value.bpm_ranges,{start:120+value.bpm_ranges.length*5,end:124+value.bpm_ranges.length*5}]})}><Plus size={14}/>Add BPM range</button>
  </div>:null}
  <label className="setting-row"><div><strong>By release date</strong><p>Year and month folders with all tracks, Warm-Up (energy 1–5), Dancefloor (6–7) and Anthems (8–10). Tracks need a release month and year.</p></div><input aria-label="By release date" type="checkbox" checked={value.release_dates} onChange={e=>toggle("release_dates",e.target.checked)}/></label>
  <label className="setting-row"><div><strong>By Genre</strong><p>One list for each genre found in your library.</p></div><input aria-label="By Genre" type="checkbox" checked={value.genres} onChange={e=>toggle("genres",e.target.checked)}/></label>
  <label className="setting-row"><div><strong>By Key</strong><p>24 Camelot lists of compatible tracks. Uses the key compatibility mode in Library preferences, with harmonic mixing as the default.</p></div><input aria-label="By Key" type="checkbox" checked={value.keys} onChange={e=>toggle("keys",e.target.checked)}/></label>
  <p className="setting-note">Lists appear under Automatic Smart Playlists. Turning an option off or removing a range keeps its existing lists. List memberships continue to update. Enabled options maintain generated list rules; duplicate a list to customize its rules.</p>
 </>;
}
