import type {Analysis,Cue,AcceptedMusicalData} from "./types";
export const playbackBpm=(analysis:Analysis|null|undefined,musical?:AcceptedMusicalData|null,position=0)=>{
 if(musical){const markers=musical.tempo_markers.filter(marker=>marker.start_seconds<=position).sort((a,b)=>b.start_seconds-a.start_seconds);return markers[0]?.bpm??(musical.source==="cuerithm_saved_publication"?analysis?.publication?.grid.bpm??analysis?.analysis.tempo?.bpm:undefined);}
 return analysis?.publication?.grid.bpm??analysis?.analysis.tempo?.bpm;
};
export function playbackCues(analysis:Analysis|null|undefined,musical?:AcceptedMusicalData|null):{cues:Cue[];emergency_loop:{time_seconds:number;loop_beats:number}|null}|null {
  if(musical)return {cues:musical.cues.map(c=>({time_seconds:c.start_seconds,name:c.name??(c.memory?"Memory cue":"Saved cue"),color:c.color,position:c.slot??undefined,memory:c.memory,kind:c.kind,end_seconds:c.kind==="loop"&&c.end_seconds!==null&&c.end_seconds>c.start_seconds?c.end_seconds:null})),emergency_loop:null};
  if(analysis?.publication)return {cues:analysis.publication.cues.map(c=>({time_seconds:c.startTime,name:c.name,color:c.color,position:c.position})),emergency_loop:null};
  return analysis?.analysis.lexicon_cues??null;
}
export function playbackBeats(analysis:Analysis|null|undefined,duration:number,musical?:AcceptedMusicalData|null):number[]{
  if(musical?.source==="cuerithm_saved_publication"&&analysis)return playbackBeats(analysis,duration);
  if(musical){const markers=[...musical.tempo_markers].sort((a,b)=>a.start_seconds-b.start_seconds);const beats:number[]=[];for(let index=0;index<markers.length;index++){const marker=markers[index],end=Math.min(duration,markers[index+1]?.start_seconds??duration);if(marker.beat_number===null||marker.beats_per_bar===null||!Number.isFinite(marker.bpm)||marker.bpm<=0)continue;const step=60/marker.bpm,count=Math.ceil((end-marker.start_seconds)/step);if(!Number.isFinite(count)||count>1_000_000||beats.length+count>1_000_000)return [];for(let i=0;i<count;i++){const value=marker.start_seconds+i*step;if(value>=0&&value<end)beats.push(value);}}return beats;}
  const grid=analysis?.publication?.grid;if(!grid)return analysis?.analysis.tempo?.beats_seconds??[];
  if(!Number.isFinite(grid.bpm)||grid.bpm<=0||!Number.isFinite(grid.startTime)||grid.startTime<0||!Number.isFinite(duration)||duration<=grid.startTime)return [];
  const step=60/grid.bpm,count=Math.ceil((duration-grid.startTime)/step);
  if(!Number.isFinite(count)||count>1_000_000)return [];
  return Array.from({length:count},(_,i)=>grid.startTime+i*step).filter(t=>t<duration);
}
