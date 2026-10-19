import type {Stop} from "../waveform-colour";
// Left-to-right colour ramp for a waveform stroke; `x2` is the width of the drawing's coordinate space.
export default function WaveGradient({id,x2,stops}:{id:string;x2:number;stops:Stop[]}){
  return <defs><linearGradient id={id} gradientUnits="userSpaceOnUse" x1="0" y1="0" x2={x2} y2="0">{stops.map((s,i)=><stop key={i} offset={s.offset} style={{stopColor:s.color}}/>)}</linearGradient></defs>;
}
