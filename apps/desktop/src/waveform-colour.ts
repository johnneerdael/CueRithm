import type {Waveform} from "./types";
export type Bands=[number,number,number];
export interface Stop{offset:number;color:string}
// Silent stretches carry no band energy, so they take the neutral text colour.
const neutral="var(--cr-muted)";
export const bandColour=(b:Bands):string=>b[0]+b[1]+b[2]===0?neutral:`rgb(${b[0]},${b[1]},${b[2]})`;
// One gradient stop per group of bins (at most `limit` stops), coloured by the group's mean bass/mid/high balance.
export function gradientStops(wave:Pick<Waveform,"bands">,first:number,last:number,offsetOf:(index:number)=>number,limit=400):Stop[]{
  const bands=wave.bands;
  if(!bands?.length||last<=first)return [];
  const size=Math.max(1,Math.ceil((last-first)/limit)),stops:Stop[]=[];
  for(let from=first;from<last;from+=size){
    const to=Math.min(last,from+size),sum:Bands=[0,0,0];
    for(let i=from;i<to;i++){const b=bands[i];if(b){sum[0]+=b[0];sum[1]+=b[1];sum[2]+=b[2];}}
    const n=to-from,offset=Math.min(1,Math.max(0,offsetOf((from+to-1)/2)));
    stops.push({offset,color:bandColour([Math.round(sum[0]/n),Math.round(sum[1]/n),Math.round(sum[2]/n)])});
  }
  return stops;
}
