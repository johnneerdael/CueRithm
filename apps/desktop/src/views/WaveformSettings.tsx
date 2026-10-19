import {useId,useMemo} from "react";
import WaveformArt from "../components/WaveformArt";
import type {Waveform} from "../types";
import {waveformStyles} from "../waveform-colour";
import type {WaveformStyle} from "../waveform-colour";
import {buildShape} from "../waveform-shape";
import {useWaveformStyle} from "../waveform-style";
// A made-up track (intro, drop, breakdown, drop) so every style shows how it reads, not just its palette.
const sections:{until:number;rms:number;low:number;mid:number;high:number}[]=[
  {until:.14,rms:.16,low:.05,mid:.14,high:.06},
  {until:.3,rms:.28,low:.32,mid:.2,high:.05},
  {until:.52,rms:.46,low:.55,mid:.22,high:.03},
  {until:.68,rms:.2,low:.03,mid:.18,high:.11},
  {until:1,rms:.48,low:.58,mid:.24,high:.04},
];
export function sampleWaveform(count=160):Waveform{
  const bins:Waveform["bins"]=[],bands:NonNullable<Waveform["bands"]>=[],rms:number[]=[],levels:NonNullable<Waveform["levels"]>=[];
  for(let i=0;i<count;i++){
    const t=i/count,s=sections.find(x=>t<x.until)!,wobble=1+.18*Math.sin(i*1.7)+.1*Math.sin(i*.43);
    const level:[number,number,number]=[s.low*wobble,s.mid*wobble,s.high*wobble],weighted=[level[0]*1.2,level[1],level[2]*4];
    const strongest=Math.max(...weighted);
    bands.push(weighted.map(v=>Math.round(v/strongest*255)) as [number,number,number]);
    levels.push(level);rms.push(Math.min(.9,s.rms*wobble));
    bins.push({min:-Math.min(1,s.rms*wobble*1.7),max:Math.min(1,s.rms*wobble*1.7)});
  }
  return {duration_seconds:300,bins,bands,rms,levels};
}
function Sample({style,wave}:{style:WaveformStyle;wave:Waveform}){
  const id="sample-"+useId().replace(/[^a-zA-Z0-9_-]/g,""),width=240,height=44;
  const shape=useMemo(()=>buildShape(wave,style,{first:0,last:wave.bins.length,columns:120,x:bin=>(bin+.5)/wave.bins.length*width,width,mid:height/2,half:height/2-5,stacked:true}),[wave,style]);
  return <svg viewBox={"0 0 "+width+" "+height} preserveAspectRatio="none" aria-hidden="true" data-wave-style={style}><WaveformArt shape={shape} id={id} width={width} barClass="preview-wave"/></svg>;
}
export default function WaveformSettings(){
  const [style,setStyle]=useWaveformStyle();
  const wave=useMemo(()=>sampleWaveform(),[]);
  return <section className="setting-section"><div className="setting-heading"><div><h2>Appearance</h2><p>How waveforms are drawn in track previews and the player. Saved on this computer and applied immediately.</p></div></div>
    <div className="wave-style-options" role="radiogroup" aria-label="Waveform style">
      {waveformStyles.map(option=><button key={option.id} type="button" role="radio" aria-checked={style===option.id} className="wave-style-option" onClick={()=>setStyle(option.id)}>
        <Sample style={option.id} wave={wave}/><strong>{option.label}</strong><span className="detail">{option.detail}</span>
      </button>)}
    </div>
    <p className="setting-note">Bar height follows the average loudness of each moment, scaled per track.</p>
  </section>;
}
