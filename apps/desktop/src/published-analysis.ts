import type {Analysis,Cue} from "./types";
export const playbackBpm=(analysis:Analysis|null|undefined)=>analysis?.publication?.grid.bpm??analysis?.analysis.tempo?.bpm;
export function playbackCues(analysis:Analysis|null|undefined):{cues:Cue[];emergency_loop:{time_seconds:number;loop_beats:number}|null}|null {
  if(analysis?.publication)return {cues:analysis.publication.cues.map(c=>({time_seconds:c.startTime,name:c.name,color:c.color,position:c.position})),emergency_loop:null};
  return analysis?.analysis.lexicon_cues??null;
}
export function playbackBeats(analysis:Analysis|null|undefined,duration:number):number[]{
  const grid=analysis?.publication?.grid;if(!grid)return analysis?.analysis.tempo?.beats_seconds??[];
  if(!Number.isFinite(grid.bpm)||grid.bpm<=0||!Number.isFinite(grid.startTime)||grid.startTime<0||!Number.isFinite(duration)||duration<=grid.startTime)return [];
  const step=60/grid.bpm,count=Math.ceil((duration-grid.startTime)/step);
  if(!Number.isFinite(count)||count>1_000_000)return [];
  return Array.from({length:count},(_,i)=>grid.startTime+i*step).filter(t=>t<duration);
}
