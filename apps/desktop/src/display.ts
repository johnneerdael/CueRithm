export function time(seconds:number|undefined|null,precise=false):string {
  if(seconds==null||!Number.isFinite(seconds))return "—";
  const value=Math.max(0,seconds);
  return Math.floor(value/60)+":"+(precise?(value%60).toFixed(1).padStart(4,"0"):Math.floor(value%60).toString().padStart(2,"0"));
}
const colors:Record<string,string>={Red:"#f26974",Blue:"#5c9ff2",Orange:"#edb267",Yellow:"#dfce62",Purple:"#b593f6",Green:"#59ceb0",Pink:"#e98ac4"};
const publishedColors:Record<string,string>={red_dark:"#9e1006",red:"#e60e0e",red_light:"#f2665c",red_orange:"#ef590f",orange:"#e88915",beige:"#ffe194",yellow_dark:"#f5d000",yellow:"#f5f50c",lime:"#bae815",green_light:"#adf55f",green:"#7fe80e",green_dark:"#4b8c06",teal:"#12de78",aqua:"#12deca",aqua_dark:"#099486",blue_light:"#2fa8ed",blue:"#1058de",blue_dark:"#0028ab",blue_violet:"#8400ff",violet:"#aa3bff",violet_light:"#c68cf3",magenta:"#e60ede",magenta_dark:"#aa00bd",magenta_red:"#de105c",grey_light:"#adadad",grey_dark:"#5c5c5c",black:"#303030",white:"#ffffff"};
// Palette names a cue template may use, in display order.
export const publishedColorNames=Object.keys(publishedColors);
export const cueColor=(color:string|null)=>color?(colors[color]??publishedColors[color]??"#00d2bf"):"#899692";

// Number/mode palette from https://dj.studio/blog/camelot-wheel (both rings).
const camelotColors:Record<string,string>={
  "1A":"#7bf4e5","1B":"#00eed3","2A":"#99f3c0","2B":"#44ed92",
  "3A":"#c2f6a6","3B":"#95f061","4A":"#ffe4a1","4B":"#ffd359",
  "5A":"#ffd1be","5B":"#ffb18e","6A":"#ffc6ca","6B":"#ff9da5",
  "7A":"#ffc3db","7B":"#ff98c1","8A":"#f7c4ec","8B":"#f29be1",
  "9A":"#e8cbff","9B":"#d7a6ff","10A":"#cfdeff","10B":"#aec6ff",
  "11A":"#a8ecfc","11B":"#65e1fb","12A":"#79f4f6","12B":"#00efef",
};
export const camelotColor=(key:string|null|undefined)=>camelotColors[key?.trim().toUpperCase()??""];
export const keyStyle=(key:string|null|undefined)=>{const color=camelotColor(key);return color?{background:color,borderColor:color,color:"#142421"}:undefined;};
