import type {Shape} from "../waveform-shape";
// The one place waveform geometry becomes SVG, shared by row previews, the player and the settings samples.
// `width` is the drawing's coordinate width, `barClass` styles the single-stroke (blended) form.
export default function WaveformArt({shape,id,width,barClass,vectorEffect}:{shape:Shape;id:string;width:number;barClass:string;vectorEffect?:"non-scaling-stroke"}){
  if(shape.mode==="layers")return <g className="wave-layers">{shape.layers.map((layer,i)=><path key={i} className="wave-layer" d={layer.path} style={{fill:layer.fill}}/>)}</g>;
  return <>
    {shape.stops.length?<defs><linearGradient id={id} gradientUnits="userSpaceOnUse" x1="0" y1="0" x2={width} y2="0">{shape.stops.map((s,i)=><stop key={i} offset={s.offset} style={{stopColor:s.color}}/>)}</linearGradient></defs>:null}
    <path className={barClass} d={shape.path} vectorEffect={vectorEffect} style={shape.stops.length?{stroke:"url(#"+id+")"}:undefined}/>
  </>;
}
