import {useId,useMemo,useState} from "react";
import {ChevronLeft,ChevronRight,Play} from "lucide-react";
import type {Track} from "../types";
import {buildPlaylistFlow} from "../playlist-flow";
import "../playlist-flow.css";

export interface PlaylistFlowProps {tracks:readonly Track[];orderedTrackIds:readonly string[];totalCount?:number;selectedTrackId?:string|null;onSelect:(id:string)=>void;onPlay:(id:string)=>void}
const pageSize=40;
const number=new Intl.NumberFormat("en",{maximumFractionDigits:2});

export default function PlaylistFlow({tracks,orderedTrackIds,totalCount,selectedTrackId,onSelect,onPlay}:PlaylistFlowProps){
 const heading=useId(),description=useId(),rows=useMemo(()=>buildPlaylistFlow(tracks,orderedTrackIds),[tracks,orderedTrackIds]);
 const [page,setPage]=useState(0),lastPage=Math.max(0,Math.ceil(rows.length/pageSize)-1),currentPage=Math.min(page,lastPage),first=currentPage*pageSize;
 const visible=rows.slice(first,first+pageSize),total=Math.max(orderedTrackIds.length,totalCount??0);
 return <section className="playlist-flow" aria-labelledby={heading} aria-describedby={description}>
  <div className="playlist-flow-heading"><h2 id={heading}>Playlist flow</h2><span>{number.format(rows.length)} of {number.format(total)} tracks loaded</span></div>
  <p id={description} className="playlist-flow-note">Playlist order · Whole-track energy · Changes from the previous track. Key matching is a preparation aid; listen before choosing a transition.</p>
  {rows.length===0?<p className="playlist-flow-empty">{total===0?"Add tracks to see this playlist’s flow.":"No track details loaded yet."}</p>:<>
   {rows.length<total?<p className="playlist-flow-partial">Partial overview. Comparisons cover loaded neighbors only.</p>:null}
   <div className="playlist-flow-columns" aria-hidden="true"><span>TRACK / ARTIST</span><span>ENERGY</span><span>BPM / CHANGE</span><span>KEY / MATCH</span><span/></div>
   <ol role="list" className="playlist-flow-list" aria-label="Tracks in playlist order">
    {visible.map(row=>{
     const {track}=row,selected=selectedTrackId===track.id;
     const comparison=row.predecessor==="start"?"Start of playlist":row.predecessor==="unloaded"?"Previous track not loaded":row.keyComparison==="match"?"Harmonic key match":row.keyComparison==="outside"?"No harmonic key match":"Key comparison unknown";
     return <li key={track.id} value={row.position} className={"playlist-flow-row"+(selected?" selected":"")}>
      <button className="playlist-flow-select" aria-label={"Select "+track.title} aria-pressed={selected} onClick={()=>onSelect(track.id)}><span className="playlist-flow-position">{row.position}</span><span className="playlist-flow-title"><strong>{track.title}</strong><small>{track.artist||"Unknown artist"}</small></span></button>
      <div className="playlist-flow-energy">{row.energy===null?<span className="playlist-flow-unknown">Energy unknown</span>:<><span>{number.format(row.energy)}/10</span><span className="playlist-flow-meter" role="meter" aria-label={"Whole-track energy for "+track.title} aria-valuemin={1} aria-valuemax={10} aria-valuenow={row.energy}><i style={{width:row.energy*10+"%"}}/></span></>}</div>
      <div className="playlist-flow-measure"><span>{row.bpm===null?"BPM unknown":number.format(row.bpm)+" BPM"}</span><small>{row.predecessor==="start"?"—":row.predecessor==="unloaded"?"Change unknown":row.bpmChange===null?"Change unknown":(row.bpmChange>0?"+":"")+number.format(row.bpmChange)+" BPM"}</small></div>
      <div className="playlist-flow-measure"><span title={track.key??undefined}>{row.keyKnown?track.key:"Key unknown"}</span><small className={row.keyComparison==="match"&&row.predecessor==="loaded"?"playlist-flow-match":""}>{comparison}</small></div>
      <button className="playlist-flow-play" aria-label={"Preview "+track.title} title="Preview in player" onClick={()=>onPlay(track.id)}><Play size={14}/></button>
     </li>;
    })}
   </ol>
   {rows.length>pageSize?<div className="playlist-flow-pagination"><span aria-live="polite">Showing loaded tracks {number.format(first+1)}–{number.format(first+visible.length)} of {number.format(rows.length)}</span><button aria-label="Previous tracks" disabled={currentPage===0} onClick={()=>setPage(currentPage-1)}><ChevronLeft size={15}/></button><button aria-label="Next tracks" disabled={currentPage===lastPage} onClick={()=>setPage(currentPage+1)}><ChevronRight size={15}/></button></div>:null}
  </>}
 </section>;
}
