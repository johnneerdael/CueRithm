export function time(seconds:number|undefined|null,precise=false):string {
  if(seconds==null||!Number.isFinite(seconds))return "—";
  const value=Math.max(0,seconds);
  return Math.floor(value/60)+":"+(precise?(value%60).toFixed(1).padStart(4,"0"):Math.floor(value%60).toString().padStart(2,"0"));
}
const colors:Record<string,string>={Red:"#f26974",Blue:"#5c9ff2",Orange:"#edb267",Yellow:"#dfce62",Purple:"#b593f6",Green:"#59ceb0",Pink:"#e98ac4"};
export const cueColor=(color:string)=>colors[color]??"#00d2bf";
