import {useEffect,useId,useMemo,useState} from "react";
import type {Cue,Detail,Waveform} from "../types";
import {cueColor,cueSlot} from "../display";
import {playbackCues} from "../published-analysis";
import {buildShape} from "../waveform-shape";
import {useWaveformStyle} from "../waveform-style";
import {useAppliedTheme} from "../theme";
import WaveformArt from "./WaveformArt";
// A loader that rejects says which step failed: the waveform from the audio, or the request for the track itself.
// Anything else it throws is treated as a request failure, because that wording claims the least.
export class PreviewLoadError extends Error{
  constructor(readonly kind:"audio"|"request",cause:unknown){super(cause instanceof Error?cause.message:String(cause));this.name="PreviewLoadError";}
}
export type PreviewLoader=(id:string)=>Promise<{wave:Waveform;detail:Detail}>;
export const FILE_NOT_FOUND="File not found - move it back or run Maintenance";
/** Thrown by a loader when the track's file is definitely gone, so the row can say so instead of showing a bare dash. */
export class FileNotFoundError extends Error{constructor(){super(FILE_NOT_FOUND);this.name="FileNotFoundError";}}
interface Preview{duration:number;wave:Waveform;grid:number[];cues:Cue[]}
const columns=120,width=240,height=36,phraseBars=16,settle=160,concurrency=2;
let running=0;const waiting:{start:()=>void;drop:()=>void}[]=[];
// Work that has not started yet can be dropped, so rows scrolled past never decode their track.
function slot<T>(work:()=>Promise<T>):{promise:Promise<T>;cancel:()=>boolean}{
  let item:{start:()=>void;drop:()=>void}|undefined;
  const promise=new Promise<T>((resolve,reject)=>{
    const start=()=>{running++;work().then(resolve,reject).finally(()=>{running--;waiting.shift()?.start();});};
    if(running<concurrency)start();else{item={start,drop:()=>reject(new Error("cancelled"))};waiting.push(item);}
  });
  const cancel=()=>{const at=item?waiting.indexOf(item):-1;if(at<0)return false;waiting.splice(at,1)[0].drop();return true;};
  return {promise,cancel};
}
// A dash says why there is no chart. It only claims what the load tells us: the file is gone, the audio step failed, the request failed, or the waveform is empty.
const unavailable={missing:FILE_NOT_FOUND,audio:"No preview: the audio file could not be read. It may be missing, moved or in an unsupported format.",request:"No preview: it could not be loaded.",empty:"No preview: this track has no waveform data.",none:"No preview available."};
const dash=(reason:string)=><span className="dim" role="img" aria-label={reason} title={reason}>—</span>;
// A file the library knows is gone keeps its own message; a step that failed says which step; anything else claims the least.
const failure=(error:unknown):"audio"|"request"|"missing"=>error instanceof FileNotFoundError?"missing":error instanceof PreviewLoadError?error.kind:"request";
function build(wave:Waveform,detail:Detail):Preview{
  const analysis=detail.analysis?.analysis,tempo=analysis?.tempo;
  const bars=tempo?.downbeats_seconds??tempo?.beats_seconds.filter((_,i)=>i%4===0)??[];
  return {duration:wave.duration_seconds||analysis?.duration_seconds||0,wave,grid:bars.filter((_,i)=>i%phraseBars===0),cues:playbackCues(detail.analysis)?.cues??[]};
}
// Cached per source identity so rows can unmount and remount while scrolling without refetching.
// Each caller holds a reference; when the last one lets go before the work started, the work is dropped.
interface Entry{promise:Promise<Preview>;refs:number;cancel:()=>boolean}
const entries=new WeakMap<PreviewLoader,Map<string,Entry>>();
function fetchPreview(load:PreviewLoader,id:string,identity:string){
  let cache=entries.get(load);if(!cache){cache=new Map();entries.set(load,cache);}
  const key=id+":"+identity;let entry=cache.get(key);
  if(!entry){
    const queued=slot(()=>load(id));
    const created:Entry={promise:queued.promise.then(({wave,detail})=>build(wave,detail)),refs:0,cancel:queued.cancel};
    created.promise.catch(()=>{if(cache.get(key)===created)cache.delete(key);});
    cache.set(key,created);if(cache.size>400)cache.delete(cache.keys().next().value!);
    entry=created;
  }
  const held=entry;held.refs++;
  const release=()=>{if(--held.refs<=0)held.cancel();};
  return {promise:held.promise,release};
}
export default function RowPreview({id,identity,load}:{id:string;identity:string;load?:PreviewLoader}){
  const [data,setData]=useState<Preview|null>(null),[failed,setFailed]=useState<"audio"|"request"|"missing"|null>(null);
  const [style]=useWaveformStyle();
  const theme=useAppliedTheme();
  const gradient="wave-"+useId().replace(/[^a-zA-Z0-9_-]/g,"");
  useEffect(()=>{
    setData(null);setFailed(null);
    if(!load)return;
    let current=true;
    let release=()=>{};
    const timer=setTimeout(()=>{const job=fetchPreview(load,id,identity);release=job.release;job.promise.then(value=>{if(current)setData(value);}).catch(error=>{if(current)setFailed(failure(error));});},settle);
    return()=>{current=false;clearTimeout(timer);release();};
  },[id,identity,load]);
  const shape=useMemo(()=>data?buildShape(data.wave,style,{first:0,last:data.wave.bins.length,columns,x:bin=>(bin+.5)/data.wave.bins.length*width,width,mid:height/2,half:height/2-5,stacked:true,theme}):null,[data,style,theme]);
  if(failed==="missing")return dash(unavailable.missing);
  if(!load)return dash(unavailable.none);
  if(failed)return dash(unavailable[failed]);
  if(data&&!data.wave.bins.length)return dash(unavailable.empty);
  if(!data||!shape)return <span className="row-preview-chart loading" aria-hidden="true"/>;
  const x=(t:number)=>Math.max(0,Math.min(width,t/Math.max(.001,data.duration)*width));
  return <span className="row-preview-chart" data-wave-style={style} role="img" aria-label={data.cues.length+(data.cues.length===1?" cue":" cues")} title={data.cues.map((c,i)=>cueSlot(c.position??i)+" · "+c.name).join("\n")||"No cues"}>
    <svg viewBox={"0 0 "+width+" "+height} preserveAspectRatio="none" aria-hidden="true">
      {data.grid.map((t,i)=><line key={i} className="preview-grid" x1={x(t)} x2={x(t)} y1="0" y2={height}/>)}
      <WaveformArt shape={shape} id={gradient} width={width} barClass="preview-wave"/>
      {data.cues.map((c,i)=><g key={i} data-cue={c.name}><line x1={x(c.time_seconds)} x2={x(c.time_seconds)} y1="0" y2={height} stroke={cueColor(c.color)} opacity=".55"/><path d={"M"+(x(c.time_seconds)-4)+",0h8l-4,7z"} fill={cueColor(c.color)}/></g>)}
    </svg>
  </span>;
}
