import type {Track} from "./types";

export type FlowKeyComparison="match"|"outside"|"unknown";
export interface PlaylistFlowRow {
 track:Track;
 position:number;
 energy:number|null;
 bpm:number|null;
 bpmChange:number|null;
 keyKnown:boolean;
 predecessor:"start"|"loaded"|"unloaded";
 keyComparison:FlowKeyComparison;
}

// Match cuerithm-smartlists/src/keys.rs: Camelot, Open Key and musical aliases.
function flowKey(value:string|null):{number:number;minor:boolean}|null {
 if(!value)return null;
 const text=value.trim().toLowerCase(),wheel=/^(\d{1,2})([abdm])$/.exec(text);
 if(wheel){
  const number=Number(wheel[1]),mode=wheel[2];
  if(number<1||number>12)return null;
  return {number:mode==="d"||mode==="m"?(number+6)%12+1:number,minor:mode==="a"||mode==="m"};
 }
 const minor=/( minor|min|m)$/.test(text),note=text.replace(/( minor|min|m| major|maj)$/,""),tonics:Record<string,number>={c:0,"c#":1,db:1,d:2,"d#":3,eb:3,e:4,f:5,"f#":6,gb:6,g:7,"g#":8,ab:8,a:9,"a#":10,bb:10,b:11};
 const tonic=tonics[note];
 if(tonic===undefined)return null;
 return {number:(minor?[5,12,7,2,9,4,11,6,1,8,3,10]:[8,3,10,5,12,7,2,9,4,11,6,1])[tonic],minor};
}

export function compareFlowKeys(left:string|null,right:string|null):FlowKeyComparison {
 const a=flowKey(left),b=flowKey(right);
 if(!a||!b)return "unknown";
 const distance=Math.abs(a.number-b.number);
 return distance===0||(a.minor===b.minor&&(distance===1||distance===11))?"match":"outside";
}

function bpm(value:number|null):number|null {return value!==null&&Number.isFinite(value)&&value>0?value:null;}
function energy(value:number|null):number|null {return value!==null&&Number.isFinite(value)&&value>=1&&value<=10?value:null;}

/** Canonical IDs, never the track table's temporary sort, define adjacency. */
export function buildPlaylistFlow(tracks:readonly Track[],orderedTrackIds:readonly string[]):PlaylistFlowRow[] {
 const byId=new Map(tracks.map(track=>[track.id,track])),rows:PlaylistFlowRow[]=[];
 orderedTrackIds.forEach((id,index)=>{
  const track=byId.get(id);
  if(!track)return;
  const previous=index>0?byId.get(orderedTrackIds[index-1]):undefined,currentBpm=bpm(track.bpm),previousBpm=previous?bpm(previous.bpm):null;
  rows.push({track,position:index+1,energy:energy(track.energy),bpm:currentBpm,keyKnown:flowKey(track.key)!==null,
   bpmChange:currentBpm!==null&&previousBpm!==null?Math.round((currentBpm-previousBpm)*100)/100:null,
   predecessor:index===0?"start":previous?"loaded":"unloaded",
   keyComparison:previous?compareFlowKeys(previous.key,track.key):"unknown"});
 });
 return rows;
}
