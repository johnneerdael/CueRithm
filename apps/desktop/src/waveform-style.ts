import {useSyncExternalStore} from "react";
import {defaultStyle,waveformStyles} from "./waveform-colour";
import type {WaveformStyle} from "./waveform-colour";
const key="cuerithm-waveform-style";
const fallback:WaveformStyle=defaultStyle;
const valid=(value:unknown):value is WaveformStyle=>waveformStyles.some(s=>s.id===value);
function stored():WaveformStyle{try{const v=localStorage.getItem(key);return valid(v)?v:fallback;}catch{return fallback;}}
let current=stored();
const listeners=new Set<()=>void>();
export function setWaveformStyle(style:WaveformStyle){
  if(!valid(style)||style===current)return;
  current=style;
  try{localStorage.setItem(key,style);}catch{}
  listeners.forEach(listener=>listener());
}
// Every waveform on screen (row previews, player, settings samples) follows the same choice.
export function useWaveformStyle():[WaveformStyle,(style:WaveformStyle)=>void]{
  const style=useSyncExternalStore(listener=>{listeners.add(listener);return()=>{listeners.delete(listener);};},()=>current,()=>fallback);
  return [style,setWaveformStyle];
}
