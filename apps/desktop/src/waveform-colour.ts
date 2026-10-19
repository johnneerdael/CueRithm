export type WaveformStyle="cuerithm"|"dusk"|"lagoon"|"tonal"|"layers"|"layers-tonal";
export type Theme="light"|"dark";
/** Per-moment weight of bass, mid and high energy, 0 to 255. */
export type Bands=[number,number,number];
export interface Stop{offset:number;color:string}
type Anchors=[string,string,string];
interface Palette{id:WaveformStyle;label:string;detail:string;mode:"blend"|"layers";anchors:Anchors}
// Anchors run bass, mid, high. Every palette steps from dark to light in that order, so the bands stay
// distinguishable by lightness alone (colour-blind safe), and the default pairs the brand's cobalt and coral
// (a blue-orange axis that survives red-green colour blindness).
export const palettes:Palette[]=[
  {id:"cuerithm",label:"CueRithm",detail:"Cobalt bass, coral mids, ice highs. Moments blend between them.",mode:"blend",anchors:["#3658F5","#FF785E","#9AF0DC"]},
  {id:"dusk",label:"Dusk",detail:"Raspberry bass, amber mids, cream highs. A warm blend.",mode:"blend",anchors:["#A3246B","#FF9A3C","#FFE9B5"]},
  {id:"lagoon",label:"Lagoon",detail:"Deep blue bass, teal mids, pale mint highs. A cool blend.",mode:"blend",anchors:["#0F6CBD","#14C2A8","#D4FFF0"]},
  {id:"tonal",label:"Tonal",detail:"One blue throughout; lightness alone separates bass, mids and highs.",mode:"blend",anchors:["#2946D8","#6F86FF","#C9D3FF"]},
  {id:"layers",label:"Layers",detail:"Bass, mids and highs as stacked layers in cobalt, coral and ice.",mode:"layers",anchors:["#3658F5","#FF785E","#9AF0DC"]},
  {id:"layers-tonal",label:"Layers, tonal",detail:"Stacked layers in three tones of one blue.",mode:"layers",anchors:["#2946D8","#6F86FF","#C9D3FF"]},
];
export const waveformStyles=palettes;
export const defaultStyle:WaveformStyle="cuerithm";
const paletteOf=(style:WaveformStyle)=>palettes.find(p=>p.id===style)??palettes[0];
export const isLayered=(style:WaveformStyle)=>paletteOf(style).mode==="layers";
const rgbOfHex=(hex:string)=>[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16));

// The waveform sits on --cr-raised, which differs per theme.
const backgrounds:Record<Theme,number[]>={light:[240,241,247],dark:[34,38,53]};
const linear=(c:number)=>{const v=c/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4};
const encode=(v:number)=>255*(v<=.0031308?12.92*v:1.055*v**(1/2.4)-.055);
const luminance=(c:number[])=>.2126*linear(c[0])+.7152*linear(c[1])+.0722*linear(c[2]);
export function contrastRatio(a:number[],b:number[]){const [hi,lo]=[luminance(a),luminance(b)].sort((x,y)=>y-x);return (hi+.05)/(lo+.05);}
// OKLab is perceptually even, so mixing anchors there gives clean blends and lightness steps mean what they say.
export function toOklab(c:number[]):[number,number,number]{
  const [r,g,b]=c.map(linear);
  const l=Math.cbrt(.4122214708*r+.5363325363*g+.0514459929*b),m=Math.cbrt(.2119034982*r+.6806995451*g+.1073969566*b),s=Math.cbrt(.0883024619*r+.2817188376*g+.6299787005*b);
  return [.2104542553*l+.793617785*m-.0040720468*s,1.9779984951*l-2.428592205*m+.4505937099*s,.0259040371*l+.7827717662*m-.808675766*s];
}
function fromOklab(L:number,a:number,k:number):number[]{
  const l=(L+.3963377774*a+.2158037573*k)**3,m=(L-.1055613458*a-.0638541728*k)**3,s=(L-.0894841775*a-1.291485548*k)**3;
  return [4.0767416621*l-3.3077115913*m+.2309699292*s,-1.2684380046*l+2.6097574011*m-.3413193965*s,-.0041960863*l-.7034186147*m+1.707614701*s];
}
const toByte=(v:number)=>Math.round(Math.min(255,Math.max(0,encode(Math.min(1,Math.max(0,v))))));
const inGamut=(c:number[])=>c.every(v=>v>=-.0005&&v<=1.0005);
// Keeps hue and as much chroma as still fits in sRGB at the given lightness.
function fitOklch(L:number,C:number,h:number):number[]{
  let chroma=C,rgb=fromOklab(L,chroma*Math.cos(h),chroma*Math.sin(h));
  for(let n=0;n<14&&!inGamut(rgb);n++){chroma*=.9;rgb=fromOklab(L,chroma*Math.cos(h),chroma*Math.sin(h));}
  return rgb.map(toByte);
}
// Moves lightness away from the background (darker on light, lighter on dark) until the colour reaches
// `target`:1 contrast, keeping hue. Colours that already pass are returned untouched.
const legibleCache=new Map<string,number[]>();
export function legible(rgb:number[],theme:Theme,target=3):number[]{
  const background=backgrounds[theme];
  if(contrastRatio(rgb,background)>=target)return rgb;
  const key=theme+rgb.join(","),known=legibleCache.get(key);if(known)return known;
  const [L,a,k]=toOklab(rgb),C=Math.hypot(a,k),h=Math.atan2(k,a),room=theme==="light"?L:1-L;
  let best=rgb;
  for(let step=1;step<=60;step++){
    best=fitOklch(L+(theme==="light"?-1:1)*room*step/60,C,h);
    if(contrastRatio(best,background)>=target)break;
  }
  legibleCache.set(key,best);
  return best;
}
const css=(c:number[])=>`rgb(${c[0]},${c[1]},${c[2]})`;
// Silent stretches carry no band energy, so they take the neutral text colour.
const neutral="var(--cr-muted)";
// Dominant bands pull harder than minor ones, so a bass-heavy moment stays clearly blue rather than greying out.
const sharpen=1.6;
/** Mixes the style's anchors by band weight in OKLab. `theme` makes the result legible on that theme's background. */
export function bandColour(style:WaveformStyle,bands:Bands,theme?:Theme):string{
  const weights=bands.map(v=>Math.max(0,v/255)**sharpen),total=weights[0]+weights[1]+weights[2];
  if(total<=0)return neutral;
  const labs=paletteOf(style).anchors.map(a=>toOklab(rgbOfHex(a)));
  const mix=[0,1,2].map(i=>labs.reduce((sum,lab,k)=>sum+lab[i]*weights[k],0)/total);
  // Opposite hues (cobalt and coral) cancel towards grey when averaged, so the mix keeps its hue and lightness
  // but regains the weighted saturation of the anchors it came from. The boost is capped to stay continuous.
  const expected=labs.reduce((sum,lab,k)=>sum+Math.hypot(lab[1],lab[2])*weights[k],0)/total,length=Math.hypot(mix[1],mix[2]);
  const boost=length>0?Math.min(2.2,Math.max(1,expected/length)):1;
  const [L,a,k]=mix,rgb=fitOklch(L,Math.hypot(a,k)*boost,Math.atan2(k,a));
  return css(theme?legible(rgb,theme):rgb);
}
/** Fill colours of the bass, mid and high layers for the layered styles. */
export function layerFills(style:WaveformStyle,theme?:Theme):[string,string,string]{
  const [low,mid,high]=paletteOf(style).anchors.map(rgbOfHex);
  return [low,mid,high].map(c=>css(theme?legible(c,theme):c)) as [string,string,string];
}
