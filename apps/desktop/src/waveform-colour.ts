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
export type Theme="light"|"dark";
// The waveform sits on --cr-raised, which differs per theme.
const backgrounds:Record<Theme,number[]>={light:[240,241,247],dark:[34,38,53]};
const linear=(c:number)=>{const v=c/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4};
const encode=(v:number)=>255*(v<=.0031308?12.92*v:1.055*v**(1/2.4)-.055);
const luminance=(c:number[])=>.2126*linear(c[0])+.7152*linear(c[1])+.0722*linear(c[2]);
export function contrastRatio(a:number[],b:number[]){const [hi,lo]=[luminance(a),luminance(b)].sort((x,y)=>y-x);return (hi+.05)/(lo+.05);}
// OKLCH keeps hue and chroma steady while lightness moves, so a pale yellow darkens to gold, not grey.
function toOklch(c:number[]):[number,number,number]{
  const [r,g,b]=c.map(linear);
  const l=Math.cbrt(.4122214708*r+.5363325363*g+.0514459929*b),m=Math.cbrt(.2119034982*r+.6806995451*g+.1073969566*b),s=Math.cbrt(.0883024619*r+.2817188376*g+.6299787005*b);
  const L=.2104542553*l+.793617785*m-.0040720468*s,a=1.9779984951*l-2.428592205*m+.4505937099*s,k=.0259040371*l+.7827717662*m-.808675766*s;
  return [L,Math.hypot(a,k),Math.atan2(k,a)];
}
function fromOklch(L:number,C:number,h:number):number[]{
  const a=C*Math.cos(h),k=C*Math.sin(h);
  const l=(L+.3963377774*a+.2158037573*k)**3,m=(L-.1055613458*a-.0638541728*k)**3,s=(L-.0894841775*a-1.291485548*k)**3;
  return [4.0767416621*l-3.3077115913*m+.2309699292*s,-1.2684380046*l+2.6097574011*m-.3413193965*s,-.0041960863*l-.7034186147*m+1.707614701*s];
}
const inGamut=(c:number[])=>c.every(v=>v>=-.0005&&v<=1.0005);
// Moves lightness away from the background (darker on light, lighter on dark) until the colour reaches
// `target`:1 contrast, keeping hue and as much chroma as still fits in sRGB. Legible colours are untouched.
const legibleCache=new Map<string,number[]>();
export function legible(rgb:number[],theme:Theme,target=3):number[]{
  const background=backgrounds[theme];
  if(contrastRatio(rgb,background)>=target)return rgb;
  const key=theme+rgb.join(","),known=legibleCache.get(key);if(known)return known;
  const [L,C,h]=toOklch(rgb),room=theme==="light"?L:1-L;
  let best=rgb;
  for(let step=1;step<=60;step++){
    const lightness=L+(theme==="light"?-1:1)*room*step/60;
    let chroma=C,linearRgb=fromOklch(lightness,chroma,h);
    for(let n=0;n<14&&!inGamut(linearRgb);n++){chroma*=.9;linearRgb=fromOklch(lightness,chroma,h);}
    best=linearRgb.map(v=>Math.round(Math.min(255,Math.max(0,encode(Math.min(1,Math.max(0,v)))))));
    if(contrastRatio(best,background)>=target)break;
  }
  legibleCache.set(key,best);
  return best;
}
// Silent stretches carry no band energy, so they take the neutral text colour.
const neutral="var(--cr-muted)";
/** `theme` makes the colour legible on that theme's background; without it the raw Lexicon colour is returned. */
export function bandColour(style:WaveformStyle,bands:Bands,theme?:Theme):string{
  if(bands[0]+bands[1]+bands[2]===0)return neutral;
  const raw=filtered(isThreeBand(style)?"rgb":style as "rgb"|"blue-green"|"pink-blue",bands).map(v=>Math.round(v*255));
  const [r,g,b]=theme?legible(raw,theme):raw;
  return `rgb(${r},${g},${b})`;
}
// Layer colours for the 3-band styles; the high layer follows the theme through a token.
const layerBase:Record<"3band-blueyellow"|"3band-bluered",[number[],number[],string]>={
  "3band-blueyellow":[[7,73,184],[191,140,38],"var(--cr-wave-high-warm)"],
  "3band-bluered":[[0,96,206],[254,63,17],"var(--cr-wave-high-cool)"],
};
export function layerFills(style:"3band-blueyellow"|"3band-bluered",theme?:Theme):[string,string,string]{
  const [low,mid,high]=layerBase[style],paint=(c:number[])=>{const [r,g,b]=theme?legible(c,theme):c;return `rgb(${r},${g},${b})`;};
  return [paint(low),paint(mid),high];
}
