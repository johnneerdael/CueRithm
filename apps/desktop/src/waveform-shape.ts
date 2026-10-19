import type {Waveform} from "./types";
import {bandColour,isLayered,layerFills} from "./waveform-colour";
import type {Bands,Stop,Theme,WaveformStyle} from "./waveform-colour";
export interface Layer{path:string;fill:string}
export type Shape={mode:"bars";path:string;stops:Stop[]}|{mode:"layers";layers:Layer[]};
export interface Frame{
  first:number;last:number;
  /** Most columns to draw; bins are grouped to fit. */
  columns:number;
  /** Horizontal position of a (fractional) bin index. */
  x:(bin:number)=>number;
  /** Width of the drawing space, used to place gradient stops. */
  width:number;
  mid:number;half:number;
  /** 3-band styles: stack the layers up from the bottom edge (overviews) instead of overlaying them around the centre (zoom). */
  stacked?:boolean;
  /** Makes colours legible on this theme; omitted, the raw Lexicon colours are used. */
  theme?:Theme;
}
const fill=.92,clamp=(v:number,low:number,high:number)=>Math.min(high,Math.max(low,v));
// Lexicon's 3-band constants. Overviews stack low, mid and high from the bottom edge with heights of
// average level x 1.8 (at most half the height), x 1.6 (at most 30%) and x 0.4 after the 3x high boost.
// Zooms overlay the layers around the centre: peak x 1.1 (clamped 0.8), x 0.6 (0.7) and x 0.2 (0.5).
const mean=.9,stackLow=1.8*mean,stackMid=1.6*mean,stackHigh=.4*3*mean,peak=1.4;
const scales=new WeakMap<Waveform,{bars:number;stack:number;overlay:number;refs:Bands}>();
const percentile=(values:number[],p:number)=>{if(!values.length)return 0;const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.min(sorted.length-1,Math.floor(p*sorted.length))];};
// Scaled per track so quiet and loud tracks both use the height, and every view of one track agrees.
function scaleOf(wave:Waveform){
  let known=scales.get(wave);
  if(!known){
    const bars=percentile(wave.rms??[],.98),bass=percentile((wave.levels??[]).map(l=>l[0]),.98);
    known={
      bars:bars>0?fill/bars:1,
      stack:bass>0?clamp(.45/(bass*stackLow),.7,2.5):1,
      overlay:bass>0?clamp(.7/(bass*peak*1.1),.7,2.5):1,
      // Highs are always far quieter than bass, so each band is judged against its own typical level in this track.
      refs:[0,1,2].map(k=>Math.max(48,percentile((wave.bands??[]).map(b=>b[k]),.9))) as Bands,
    };
    scales.set(wave,known);
  }
  return known;
}
interface Column{x:number;min:number;max:number;rms:number;levels:[number,number,number];bands:Bands}
function columnsOf(wave:Waveform,frame:Frame):Column[]{
  const span=frame.last-frame.first;if(span<=0)return [];
  const count=Math.min(frame.columns,span),out:Column[]=[];
  for(let c=0;c<count;c++){
    const from=frame.first+Math.floor(c*span/count),to=Math.max(from+1,frame.first+Math.floor((c+1)*span/count));
    let min=0,max=0,rms=0,n=0;const levels:[number,number,number]=[0,0,0],bands:Bands=[0,0,0];
    for(let i=from;i<to&&i<wave.bins.length;i++){
      const bin=wave.bins[i];min=Math.min(min,bin.min);max=Math.max(max,bin.max);
      const r=wave.rms?.[i]??0;rms+=r*r;
      const l=wave.levels?.[i],b=wave.bands?.[i];
      if(l){levels[0]+=l[0]*l[0];levels[1]+=l[1]*l[1];levels[2]+=l[2]*l[2];}
      if(b){bands[0]+=b[0];bands[1]+=b[1];bands[2]+=b[2];}
      n++;
    }
    if(!n)continue;
    out.push({x:frame.x((from+to-1)/2),min,max,rms:Math.sqrt(rms/n),levels:[Math.sqrt(levels[0]/n),Math.sqrt(levels[1]/n),Math.sqrt(levels[2]/n)],bands:[Math.round(bands[0]/n),Math.round(bands[1]/n),Math.round(bands[2]/n)]});
  }
  return out;
}
const f=(v:number)=>v.toFixed(1);
// A filled band between two edges that share the columns' x positions.
const band=(columns:Column[],upper:number[],lower:number[])=>"M"+columns.map((c,i)=>f(c.x)+","+f(upper[i])).join("L")+"L"+columns.map((c,i)=>f(c.x)+","+f(lower[i])).reverse().join("L")+"Z";
function layersOf(wave:Waveform,style:WaveformStyle,columns:Column[],frame:Frame):Shape{
  const fills=layerFills(style,frame.theme),scale=scaleOf(wave),{mid,half}=frame;
  if(frame.stacked){
    // Heights are fractions of the full frame height, stacked upwards from the bottom edge.
    const bottom=mid+half,height=2*half,g=scale.stack;
    const low=columns.map(c=>Math.max(.01,Math.min(.5,c.levels[0]*stackLow*g)));
    const middle=columns.map((c,i)=>Math.min(.3,c.levels[1]*stackMid*g)+low[i]);
    const high=columns.map((c,i)=>Math.min(1,middle[i]+c.levels[2]*stackHigh*g));
    const edge=(v:number[])=>v.map(h=>bottom-h*height);
    return {mode:"layers",layers:[
      {path:band(columns,edge(low),columns.map(()=>bottom)),fill:fills[0]},
      {path:band(columns,edge(middle),edge(low)),fill:fills[1]},
      {path:band(columns,edge(high),edge(middle)),fill:fills[2]},
    ]};
  }
  const g=scale.overlay;
  const heights=[
    columns.map(c=>Math.max(.01,Math.min(.8,c.levels[0]*peak*g))*1.1),
    columns.map(c=>Math.min(.7,c.levels[1]*peak*g)*.6),
    columns.map(c=>Math.min(.5,c.levels[2]*peak*3*g)*.2),
  ];
  return {mode:"layers",layers:heights.map((h,k)=>({path:band(columns,h.map(v=>mid-v*half),h.map(v=>mid+v*half)),fill:fills[k]}))};
}
export function buildShape(wave:Waveform,style:WaveformStyle,frame:Frame):Shape{
  const columns=columnsOf(wave,frame),scale=scaleOf(wave),{mid,half}=frame;
  if(isLayered(style)&&wave.levels?.length&&columns.length)return layersOf(wave,style,columns,frame);
  const average=!!wave.rms?.length,pieces:string[]=[],stops:Stop[]=[];
  const colourStyle=isLayered(style)?(style==="layers-tonal"?"tonal":"cuerithm"):style;
  for(const c of columns){
    // Without level data (a waveform from before it was measured) fall back to the peak envelope.
    const up=average?Math.max(.02,Math.min(1,c.rms*scale.bars)):Math.min(1,c.max),down=average?up:Math.min(1,-c.min);
    pieces.push("M"+f(c.x)+","+f(mid-up*half)+"V"+f(mid+down*half));
    if(wave.bands?.length)stops.push({offset:Math.min(1,Math.max(0,c.x/frame.width)),color:bandColour(colourStyle,c.bands.map((v,k)=>Math.min(255,v/scale.refs[k]*255)) as Bands,frame.theme)});
  }
  return {mode:"bars",path:pieces.join(""),stops};
}
