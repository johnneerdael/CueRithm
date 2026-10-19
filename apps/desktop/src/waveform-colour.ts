export type WaveformStyle="rgb"|"blue-green"|"pink-blue"|"3band-blueyellow"|"3band-bluered";
export type Bands=[number,number,number];
export interface Stop{offset:number;color:string}
export const waveformStyles:{id:WaveformStyle;label:string;detail:string}[]=[
  {id:"rgb",label:"RGB",detail:"Bass, mids and highs blend into one colour per moment."},
  {id:"blue-green",label:"Blue and green",detail:"The same blend shifted towards cool tones."},
  {id:"pink-blue",label:"Pink and blue",detail:"The same blend shifted towards pink and blue."},
  {id:"3band-blueyellow",label:"3-band blue and yellow",detail:"Bass, mids and highs as stacked layers."},
  {id:"3band-bluered",label:"3-band blue and red",detail:"Bass, mids and highs as stacked layers."},
];
export const isThreeBand=(style:WaveformStyle)=>style.startsWith("3band");
// The blended styles share one measurement and differ by a colour filter, applied here so every
// renderer (and WebKit, which ignores CSS filters on SVG children) produces the same colour.
type Step=["brightness"|"contrast"|"hue",number];
const filters:Record<"rgb"|"blue-green"|"pink-blue",Step[]>={
  rgb:[["hue",346]],
  "blue-green":[["brightness",.8],["contrast",1.5],["hue",115]],
  "pink-blue":[["contrast",1.2],["hue",218]],
};
const clamp=(v:number)=>Math.min(1,Math.max(0,v));
function hue(c:number[],degrees:number){
  const a=degrees*Math.PI/180,cos=Math.cos(a),sin=Math.sin(a),[r,g,b]=c;
  return [
    r*(.213+cos*.787-sin*.213)+g*(.715-cos*.715-sin*.715)+b*(.072-cos*.072+sin*.928),
    r*(.213-cos*.213+sin*.143)+g*(.715+cos*.285+sin*.14)+b*(.072-cos*.072-sin*.283),
    r*(.213-cos*.213-sin*.787)+g*(.715-cos*.715+sin*.715)+b*(.072+cos*.928+sin*.072),
  ];
}
function filtered(style:"rgb"|"blue-green"|"pink-blue",bands:Bands){
  let c=bands.map(v=>v/255);
  for(const [name,amount] of filters[style]){
    c=(name==="hue"?hue(c,amount):name==="brightness"?c.map(v=>v*amount):c.map(v=>(v-.5)*amount+.5)).map(clamp);
  }
  return c;
}
// Silent stretches carry no band energy, so they take the neutral text colour.
const neutral="var(--cr-muted)";
export function bandColour(style:WaveformStyle,bands:Bands):string{
  if(bands[0]+bands[1]+bands[2]===0)return neutral;
  const [r,g,b]=filtered(isThreeBand(style)?"rgb":style as "rgb"|"blue-green"|"pink-blue",bands).map(v=>Math.round(v*255));
  return `rgb(${r},${g},${b})`;
}
// Layer colours for the 3-band styles; the high layer follows the theme through a token.
export const layerFills:Record<"3band-blueyellow"|"3band-bluered",[string,string,string]>={
  "3band-blueyellow":["rgb(7,73,184)","rgb(191,140,38)","var(--cr-wave-high-warm)"],
  "3band-bluered":["rgb(0,96,206)","rgb(254,63,17)","var(--cr-wave-high-cool)"],
};
