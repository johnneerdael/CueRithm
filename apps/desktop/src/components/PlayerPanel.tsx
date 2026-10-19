import {useId,useMemo,useState} from "react";
import {Play,Pause,SkipBack,Volume2,LoaderCircle,Headphones} from "lucide-react";
import Artwork from "./Artwork";
import BrandMark from "./BrandMark";
import type {Playback,Detail,Waveform,Bridge} from "../types";
import {time,cueColor} from "../display";
import {playbackBpm,playbackBeats,playbackCues} from "../published-analysis";
import {buildShape} from "../waveform-shape";
import {useWaveformStyle} from "../waveform-style";
import WaveformArt from "./WaveformArt";

function Wave({data,position,start,end,beats,cues,onSeek,onToggleZoom,zoomed=false,small=false}:{data:Waveform|null;position:number;start:number;end:number;beats:number[];cues:NonNullable<Detail["analysis"]>["analysis"]["lexicon_cues"];onSeek:(time:number)=>void;onToggleZoom?:()=>void;zoomed?:boolean;small?:boolean}){
  const span=Math.max(0.001,end-start);
  // The same shape builder and style choice as the row previews, so playback shows the same waveform.
  const [style]=useWaveformStyle();
  const gradient="wave-"+useId().replace(/[^a-zA-Z0-9_-]/g,"");
  const shape=useMemo(()=>{
    if(!data||!data.bins.length)return null;
    const bins=data.bins.length,first=Math.max(0,Math.floor(start/data.duration_seconds*bins)),last=Math.min(bins,Math.ceil(end/data.duration_seconds*bins));
    return buildShape(data,style,{first,last,columns:900,x:bin=>(bin/bins*data.duration_seconds-start)/span*1000,width:1000,mid:40,half:35,stacked:small});
  },[data,style,small,start,end,span]);
  const ticks=beats.filter(t=>t>=start&&t<=end);
  return <div className={small?"wave overview":"wave detail-wave"} data-wave-style={style} role={small?"slider":"button"} aria-label={small?"Track overview":"Waveform zoom: "+(zoomed?"16 seconds":"Entire track")} aria-pressed={small?undefined:zoomed} aria-valuemin={small?0:undefined} aria-valuemax={small?data?.duration_seconds??0:undefined} aria-valuenow={small?position:undefined} tabIndex={data?0:-1}
    title={small?"Click to seek · ← / → jump one bar":"Click to show "+(zoomed?"entire track":"16 seconds")+" · ← / → jump one bar"}
    onPointerDown={event=>{if(!data||!small)return;const rect=event.currentTarget.getBoundingClientRect();onSeek(Math.max(0,Math.min(data.duration_seconds,start+(event.clientX-rect.left)/rect.width*span)));}}
    onClick={()=>{if(data&&!small)onToggleZoom?.();}}
    onKeyDown={event=>{if(!small&&(event.key==="Enter"||event.key===" ")){event.preventDefault();event.stopPropagation();if(data)onToggleZoom?.();}}}>
    <svg viewBox="0 0 1000 80" preserveAspectRatio="none" aria-hidden="true">
      {!small?ticks.map((beat,i)=><line key={i} x1={(beat-start)/span*1000} x2={(beat-start)/span*1000} y1="0" y2="80" className="beat-line"/>):null}
      {shape?<WaveformArt shape={shape} id={gradient} width={1000} barClass="wave-path" vectorEffect="non-scaling-stroke"/>:null}
      {(cues?.cues??[]).filter(c=>c.time_seconds>=start&&c.time_seconds<=end).map((c,i)=><line data-cue={c.name} key={i} x1={(c.time_seconds-start)/span*1000} x2={(c.time_seconds-start)/span*1000} y1="0" y2="80" stroke={cueColor(c.color)} strokeWidth={small?1.3:2} vectorEffect="non-scaling-stroke" opacity=".75"/>)}
      {position>=start&&position<=end?<line x1={(position-start)/span*1000} x2={(position-start)/span*1000} y1="0" y2="80" className="playhead"/>:null}
    </svg>
    {!small?(cues?.cues??[]).map((c,i)=>c.time_seconds>=start&&c.time_seconds<=end?<span aria-hidden="true" key={i} className="wave-cue-label" style={{left:Math.max(1,Math.min(99,(c.time_seconds-start)/span*100))+"%",color:cueColor(c.color)}}>{String.fromCharCode(65+(c.position??i))}</span>:null):null}
  </div>;
}
export default function PlayerPanel({status,detail,data,busy,onToggle,onPause,onSeek,onVolume,artworkLoader}:{artworkLoader?:Bridge["getArtwork"];status:Playback;detail:Detail|null;data:Waveform|null;busy:boolean;onToggle:()=>void;onPause:()=>void;onSeek:(seconds:number)=>void;onVolume:(value:number)=>void}){
  const [zoom,setZoom]=useState(0);
  const duration=status.duration_seconds||data?.duration_seconds||0;
  const center=status.position_seconds;
  const span=zoom||duration;
  const start=zoom?Math.max(0,Math.min(Math.max(0,duration-span),center-span*.35)):0;
  const end=Math.min(duration,start+span);
  const cues=useMemo(()=>playbackCues(detail?.analysis),[detail?.analysis]);
  const beats=useMemo(()=>playbackBeats(detail?.analysis,duration),[detail?.analysis,duration]);
  return <section className="player" aria-label="Music player">
    <div className="player-header">
      <div className="now-playing-art">{detail?<Artwork id={detail.track.id} identity={detail.source_sha256} load={artworkLoader} fallback={<BrandMark/>}/>:<BrandMark/>}</div>
      <div className="now-playing"><strong>{busy?"Loading track…":detail?.track.title??"Nothing playing"}</strong><small>{detail?.track.artist||"Double-click a track to load and play"}</small></div>
      {detail?<div className="player-analysis"><span><b>{playbackBpm(detail.analysis)?.toFixed(2)??"—"}</b>BPM</span><span><b className="player-key key-chip">{detail.analysis?.analysis.key?.camelot??"—"}</b>KEY</span><span><b>{detail.analysis?.analysis.energy?.level??"—"}</b>ENERGY</span></div>:null}
      <div className="transport">
        <button aria-label="Back to start" disabled={!detail} onClick={()=>onSeek(0)}><SkipBack size={17}/></button>
        <button className="play-main" aria-label={status.playing?"Pause":"Play"} disabled={!detail||busy} onClick={onToggle}>{busy?<LoaderCircle className="spin" size={21}/>:status.playing?<Pause size={21} fill="currentColor"/>:<Play size={21} fill="currentColor"/>}</button>
        <button className="cue-button" disabled={!detail} onClick={()=>{onPause();onSeek(cues?.cues[0]?.time_seconds??0);}}>CUE</button>
      </div>
      <div className="time-readout"><b>{time(status.position_seconds,true)}</b><span>−{time(Math.max(0,duration-status.position_seconds),true)}</span></div>
      <div className="player-volume"><Volume2 size={16}/><input aria-label="Volume" type="range" min={0} max={1} step={.01} value={status.volume} onChange={event=>onVolume(Number(event.target.value))}/></div>
    </div>
    <div className="waveform-area">
      {detail?<><div className="wave-toolbar"><span>{detail.analysis?"BEATGRID & CUES":"PREVIEW"}<span className="wave-subtitle">{detail.analysis?.media?.codec?.toUpperCase()??"AUDIO"}</span></span><span>{zoom?"16 seconds":"Entire track"} · click waveform to switch</span></div>
        <Wave data={data} position={center} start={start} end={end} beats={beats} cues={cues} onSeek={onSeek} zoomed={!!zoom} onToggleZoom={()=>setZoom(value=>value?0:16)}/>
        <div className="wave-time-axis"><span>{time(start,true)}</span><span>{time(end,true)}</span></div>
        <Wave small data={data} position={center} start={0} end={duration} beats={[]} cues={cues} onSeek={onSeek}/>
      </>:<div className="player-empty"><Headphones size={23}/><span>Your next track starts here.</span><small>Preview audio, inspect the beatgrid and jump between cues.</small></div>}
    </div>
    <div className="cue-strip">
      {(cues?.cues??[]).map((cue,i)=><button key={i} className="hot-cue" style={{"--cue":cueColor(cue.color)} as React.CSSProperties} onClick={()=>onSeek(cue.time_seconds)} title={"Seek to "+cue.name}><span className="cue-slot">{String.fromCharCode(65+(cue.position??i))}</span><strong>{cue.name}</strong><span>{time(cue.time_seconds,true)}</span></button>)}
      {detail&&!cues?<span className="cue-empty">Cues appear here after preparation finishes.</span>:null}
      {status.error?<span className="player-error" role="alert">{status.error}</span>:null}
    </div>
  </section>;
}
