import {useEffect,useId,useMemo,useState} from "react";
import type {Cue,Detail,Waveform} from "../types";
import {cueColor} from "../display";
import {gradientStops} from "../waveform-colour";
import type {Stop} from "../waveform-colour";
import WaveGradient from "./WaveGradient";
export type PreviewLoader=(id:string)=>Promise<{wave:Waveform;detail:Detail}>;
interface Preview{duration:number;bins:{min:number;max:number}[];grid:number[];cues:Cue[];stops:Stop[]}
const columns=120,width=240,height=36,phraseBars=16,settle=160,concurrency=2;
const caches=new WeakMap<PreviewLoader,Map<string,Promise<Preview>>>();
let running=0;const waiting:(()=>void)[]=[];
function slot<T>(work:()=>Promise<T>):Promise<T>{
  return new Promise<T>((resolve,reject)=>{
    const start=()=>{running++;work().then(resolve,reject).finally(()=>{running--;waiting.shift()?.();});};
    if(running<concurrency)start();else waiting.push(start);
  });
}
function build(wave:Waveform,detail:Detail):Preview{
  const analysis=detail.analysis?.analysis,tempo=analysis?.tempo;
  const bars=tempo?.downbeats_seconds??tempo?.beats_seconds.filter((_,i)=>i%4===0)??[];
  const bins=wave.bins,out:Preview["bins"]=[];
  for(let c=0;c<columns&&bins.length;c++){
    let lo=0,hi=0;
    for(let i=Math.floor(c*bins.length/columns);i<Math.max(Math.floor((c+1)*bins.length/columns),Math.floor(c*bins.length/columns)+1)&&i<bins.length;i++){lo=Math.min(lo,bins[i].min);hi=Math.max(hi,bins[i].max);}
    out.push({min:lo,max:hi});
  }
  return {duration:wave.duration_seconds||analysis?.duration_seconds||0,bins:out,stops:gradientStops(wave,0,bins.length,i=>(i+.5)/bins.length,columns),grid:bars.filter((_,i)=>i%phraseBars===0),cues:analysis?.lexicon_cues?.cues??[]};
}
// Cached per source identity so rows can unmount and remount while scrolling without refetching.
function fetchPreview(load:PreviewLoader,id:string,identity:string){
  let cache=caches.get(load);if(!cache){cache=new Map();caches.set(load,cache);}
  const key=id+":"+identity;let pending=cache.get(key);
  if(!pending){
    pending=slot(()=>load(id)).then(({wave,detail})=>build(wave,detail));
    pending.catch(()=>cache.delete(key));
    cache.set(key,pending);if(cache.size>400)cache.delete(cache.keys().next().value!);
  }
  return pending;
}
export default function RowPreview({id,identity,load}:{id:string;identity:string;load?:PreviewLoader}){
  const [data,setData]=useState<Preview|null>(null),[failed,setFailed]=useState(false);
  const gradient="wave-"+useId().replace(/[^a-zA-Z0-9_-]/g,"");
  useEffect(()=>{
    setData(null);setFailed(false);
    if(!load)return;
    let current=true;
    const timer=setTimeout(()=>{fetchPreview(load,id,identity).then(value=>{if(current)setData(value);}).catch(()=>{if(current)setFailed(true);});},settle);
    return()=>{current=false;clearTimeout(timer);};
  },[id,identity,load]);
  const wave=useMemo(()=>data?data.bins.map((b,i)=>{const x=(i+.5)/columns*width;return "M"+x.toFixed(1)+","+(height/2-Math.min(1,b.max)*(height/2-5)).toFixed(1)+"V"+(height/2-Math.max(-1,b.min)*(height/2-5)+.5).toFixed(1);}).join(""):"",[data]);
  if(!load||failed||(data&&!data.bins.length))return <span className="dim">—</span>;
  if(!data)return <span className="row-preview-chart loading" aria-hidden="true"/>;
  const x=(t:number)=>Math.max(0,Math.min(width,t/Math.max(.001,data.duration)*width));
  return <span className="row-preview-chart" role="img" aria-label={data.cues.length+(data.cues.length===1?" cue":" cues")} title={data.cues.map((c,i)=>String.fromCharCode(65+i)+" · "+c.name).join("\n")||"No cues"}>
    <svg viewBox={"0 0 "+width+" "+height} preserveAspectRatio="none" aria-hidden="true">
      {data.grid.map((t,i)=><line key={i} className="preview-grid" x1={x(t)} x2={x(t)} y1="0" y2={height}/>)}
      {data.stops.length?<WaveGradient id={gradient} x2={width} stops={data.stops}/>:null}
      <path className="preview-wave" d={wave} style={data.stops.length?{stroke:"url(#"+gradient+")"}:undefined}/>
      {data.cues.map((c,i)=><g key={i} data-cue={c.name}><line x1={x(c.time_seconds)} x2={x(c.time_seconds)} y1="0" y2={height} stroke={cueColor(c.color)} opacity=".55"/><path d={"M"+(x(c.time_seconds)-4)+",0h8l-4,7z"} fill={cueColor(c.color)}/></g>)}
    </svg>
  </span>;
}
