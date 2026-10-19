import {act,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,describe,expect,it,vi} from "vitest";
import PlayerPanel from "../components/PlayerPanel";
import RowPreview from "../components/RowPreview";
import type {Detail,Playback,Waveform} from "../types";
import Settings from "../views/Settings";
import WaveformSettings,{sampleWaveform} from "../views/WaveformSettings";
import {bandColour,contrastRatio,layerFills,legible,waveformStyles} from "../waveform-colour";
import type {Theme} from "../waveform-colour";
import {buildShape} from "../waveform-shape";
import type {Frame} from "../waveform-shape";
import {setWaveformStyle} from "../waveform-style";
import {fakeBridge,track} from "./bridge";

afterEach(()=>{act(()=>setWaveformStyle("rgb"));localStorage.clear();});
const frame=(columns:number,count:number):Frame=>({first:0,last:count,columns,x:bin=>(bin+.5)/count*100,width:100,mid:20,half:15});
// Vertical extent of each "M x,top V bottom" bar.
const heights=(path:string)=>[...path.matchAll(/M[\d.]+,([\d.-]+)V([\d.-]+)/g)].map(m=>Number(m[2])-Number(m[1]));
const stopColours=(root:Element)=>[...root.querySelector("linearGradient")!.querySelectorAll("stop")].map(s=>s.getAttribute("style")!.replace(/\s/g,""));
const loader=(wave:Waveform)=>async()=>({wave,detail:{} as unknown as Detail});

describe("waveform colour",()=>{
  it("applies each style's colour filter to the bass, mid and high blend",()=>{
    expect(bandColour("rgb",[255,0,0])).toBe("rgb(255,0,50)");
    expect(bandColour("pink-blue",[255,0,0])).toBe("rgb(0,75,221)");
    const green=bandColour("rgb",[0,255,0]).match(/\d+/g)!.map(Number);
    expect(green[1]).toBeGreaterThan(200);expect(green[0]).toBeLessThan(80);expect(green[2]).toBeLessThan(40);
    expect(bandColour("blue-green",[255,0,0])).not.toBe(bandColour("rgb",[255,0,0]));
  });
  it("leaves silence on the neutral text colour",()=>{
    expect(bandColour("rgb",[0,0,0])).toBe("var(--cr-muted)");
  });
});

describe("waveform legibility",()=>{
  const background:Record<Theme,number[]>={light:[240,241,247],dark:[34,38,53]};
  const rgbOf=(css:string)=>css.match(/\d+/g)!.map(Number);
  const hueOf=([r,g,b]:number[])=>{const max=Math.max(r,g,b),min=Math.min(r,g,b),d=max-min;if(d<25)return null;const h=max===r?((g-b)/d)%6:max===g?(b-r)/d+2:(r-g)/d+4;return (h*60+360)%360;};
  const hueGap=(a:number,b:number)=>Math.min(Math.abs(a-b),360-Math.abs(a-b));
  const grid=[0,85,170,255],blends=waveformStyles.filter(s=>!s.id.startsWith("3band")).map(s=>s.id);
  const triples=grid.flatMap(r=>grid.flatMap(g=>grid.map(b=>[r,g,b] as [number,number,number]))).filter(t=>t[0]+t[1]+t[2]>0);
  it("keeps every blended colour at 3:1 contrast or better on both themes",()=>{
    for(const theme of ["light","dark"] as const)for(const style of blends)for(const t of triples){
      const colour=rgbOf(bandColour(style,t,theme));
      expect(contrastRatio(colour,background[theme]),`${style} ${theme} ${t}`).toBeGreaterThanOrEqual(2.95);
    }
  });
  it("darkens pale colours on the light theme without changing their hue",()=>{
    let adjusted=0;
    for(const style of blends)for(const t of triples){
      const raw=rgbOf(bandColour(style,t)),shown=rgbOf(bandColour(style,t,"light"));
      if(raw.join()===shown.join())continue;
      adjusted++;
      const [before,after]=[hueOf(raw),hueOf(shown)];
      if(before!==null&&after!==null)expect(hueGap(before,after),`${style} ${t}`).toBeLessThan(25);
      expect(contrastRatio(shown,background.light)).toBeGreaterThan(contrastRatio(raw,background.light));
    }
    expect(adjusted).toBeGreaterThan(0);
  });
  it("leaves colours that are already legible exactly as Lexicon draws them",()=>{
    expect(bandColour("rgb",[255,0,0],"dark")).toBe(bandColour("rgb",[255,0,0]));
    expect(legible([7,73,184],"light")).toEqual([7,73,184]);
  });
  it("lifts dark colours on the dark theme instead of darkening them",()=>{
    const raw=rgbOf(bandColour("blue-green",[255,0,0])),shown=rgbOf(bandColour("blue-green",[255,0,0],"dark"));
    expect(contrastRatio(raw,background.dark)).toBeLessThan(3);
    expect(contrastRatio(shown,background.dark)).toBeGreaterThanOrEqual(2.95);
    expect(shown[1]).toBeGreaterThan(raw[1]);
  });
  it("makes the 3-band layer colours legible on the light theme and leaves the high layer to its token",()=>{
    for(const style of ["3band-blueyellow","3band-bluered"] as const){
      const [low,mid,high]=layerFills(style,"light");
      expect(contrastRatio(rgbOf(low),background.light)).toBeGreaterThanOrEqual(2.95);
      expect(contrastRatio(rgbOf(mid),background.light)).toBeGreaterThanOrEqual(2.95);
      expect(high).toMatch(/^var\(--cr-wave-high/);
    }
  });
  it("applies the theme through buildShape",()=>{
    const w:Waveform={duration_seconds:60,bins:[{min:-.5,max:.5},{min:-.5,max:.5}],rms:[.3,.3],bands:[[255,255,0],[255,255,0]],levels:[[.3,.1,.02],[.3,.1,.02]]};
    const light=buildShape(w,"rgb",{...frame(2,2),theme:"light"}),raw=buildShape(w,"rgb",frame(2,2));
    if(light.mode!=="bars"||raw.mode!=="bars")throw new Error("expected bars");
    expect(light.stops[0].color).toBe(bandColour("rgb",[255,255,0],"light"));
    expect(raw.stops[0].color).toBe(bandColour("rgb",[255,255,0]));
  });
});

describe("waveform shape",()=>{
  const wave=(rms:number[],peak:number):Waveform=>({duration_seconds:60,bins:rms.map(()=>({min:-peak,max:peak})),rms,bands:rms.map(()=>[255,0,0]),levels:rms.map(r=>[r,r/2,r/8])});
  it("sizes bars by average level, not by peak",()=>{
    // Identical peaks, different loudness: only the average level may change the bar.
    const shape=buildShape(wave([.1,.1,.4,.4],.9),"rgb",frame(4,4));
    if(shape.mode!=="bars")throw new Error("expected bars");
    const [a,b,c,d]=heights(shape.path);
    expect(a).toBeCloseTo(b);expect(c).toBeCloseTo(d);
    expect(c).toBeGreaterThan(a*2);
  });
  it("scales each track so a quiet one still fills the height",()=>{
    const quiet=buildShape(wave([.02,.04,.08,.1],.9),"rgb",frame(4,4)),loud=buildShape(wave([.2,.4,.8,1],.9),"rgb",frame(4,4));
    if(quiet.mode!=="bars"||loud.mode!=="bars")throw new Error("expected bars");
    const q=heights(quiet.path),l=heights(loud.path);
    expect(Math.max(...q)).toBeCloseTo(Math.max(...l),1);
    expect(Math.max(...q)).toBeGreaterThan(20);
  });
  it("falls back to the peak envelope when a waveform has no level data",()=>{
    const old:Waveform={duration_seconds:60,bins:[{min:-.2,max:.2},{min:-.8,max:.8}]};
    const shape=buildShape(old,"rgb",frame(2,2));
    if(shape.mode!=="bars")throw new Error("expected bars");
    const [small,large]=heights(shape.path);
    expect(large).toBeGreaterThan(small*3);
    expect(shape.stops).toEqual([]);
  });
  it("gives one gradient stop per column, coloured by the chosen style",()=>{
    const w=wave([.2,.2,.2,.2],.5),rgb=buildShape(w,"rgb",frame(4,4)),pink=buildShape(w,"pink-blue",frame(4,4));
    if(rgb.mode!=="bars"||pink.mode!=="bars")throw new Error("expected bars");
    expect(rgb.stops).toHaveLength(4);
    expect(rgb.stops[0].color).toBe(bandColour("rgb",[255,0,0]));
    expect(pink.stops[0].color).toBe(bandColour("pink-blue",[255,0,0]));
    expect(rgb.stops.map(s=>s.offset)).toEqual([...rgb.stops.map(s=>s.offset)].sort((x,y)=>x-y));
  });
  it("groups bins into at most the requested columns",()=>{
    const shape=buildShape(wave(Array(1000).fill(.3),.5),"rgb",frame(50,1000));
    if(shape.mode!=="bars")throw new Error("expected bars");
    expect(heights(shape.path)).toHaveLength(50);expect(shape.stops).toHaveLength(50);
  });
  const top=(path:string)=>Number(path.match(/^M[\d.]+,([\d.-]+)/)![1]);
  const lowest=(path:string)=>Math.max(...[...path.matchAll(/,([\d.-]+)/g)].map(m=>Number(m[1])));
  it("overlays the 3-band layers around the centre for zoomed views, bass widest and the high layer a thin core",()=>{
    const shape=buildShape(wave([.3,.3,.3,.3],.6),"3band-blueyellow",frame(4,4));
    if(shape.mode!=="layers")throw new Error("expected layers");
    expect(shape.layers.map(l=>l.fill)).toEqual(layerFills("3band-blueyellow"));
    // Smaller y is further above the centre line (20): bass reaches furthest, then mids, then highs.
    const [bass,mids,highs]=shape.layers.map(l=>20-top(l.path));
    expect(bass).toBeGreaterThan(mids);expect(mids).toBeGreaterThan(highs);
    expect(highs).toBeLessThan(bass/5);
    expect(highs).toBeGreaterThan(0);
  });
  it("stacks the 3-band layers up from the bottom edge for overviews: bass, then mids, then highs",()=>{
    const shape=buildShape(wave([.3,.3,.3,.3],.6),"3band-bluered",{...frame(4,4),stacked:true});
    if(shape.mode!=="layers")throw new Error("expected layers");
    expect(shape.layers.map(l=>l.fill)).toEqual(layerFills("3band-bluered"));
    // The base layer sits on the bottom edge (mid 20 + half 15) and each layer starts where the last ended.
    expect(lowest(shape.layers[0].path)).toBeCloseTo(35);
    const [bass,mids,highs]=shape.layers.map(l=>top(l.path));
    expect(bass).toBeLessThan(35);
    expect(mids).toBeLessThan(bass);expect(highs).toBeLessThan(mids);
    expect(lowest(shape.layers[1].path)).toBeCloseTo(bass);
    expect(lowest(shape.layers[2].path)).toBeCloseTo(mids);
  });
  it("caps the stacked bass at half the height and the mids at 30% so a loud track cannot swallow the highs",()=>{
    const loud:Waveform={...wave([.9,.9],.9),levels:[[.9,.9,.1],[.9,.9,.1]]};
    const shape=buildShape(loud,"3band-blueyellow",{...frame(2,2),stacked:true});
    if(shape.mode!=="layers")throw new Error("expected layers");
    const height=30,[bass,mids]=shape.layers.map(l=>top(l.path));
    expect((35-bass)/height).toBeCloseTo(.5,1);
    expect((bass-mids)/height).toBeCloseTo(.3,1);
  });
});

describe("waveform styles in the app",()=>{
  it("strokes a row preview with the colours of the chosen style and switches live",async()=>{
    const {container}=render(<RowPreview id="t" identity="a" load={loader(sampleWaveform(120))}/>);
    await waitFor(()=>expect(container.querySelector("linearGradient")).not.toBeNull(),{timeout:2000});
    const rgb=stopColours(container);
    expect(container.querySelector(".preview-wave")!.getAttribute("style")).toContain("url(#"+container.querySelector("linearGradient")!.id+")");
    act(()=>setWaveformStyle("pink-blue"));
    expect(container.querySelector(".row-preview-chart")).toHaveAttribute("data-wave-style","pink-blue");
    expect(stopColours(container)).not.toEqual(rgb);
    act(()=>setWaveformStyle("3band-bluered"));
    expect(container.querySelector("linearGradient")).toBeNull();
    expect(container.querySelectorAll(".wave-layer")).toHaveLength(3);
  });
  it("keeps the single accent stroke for a waveform without band data",async()=>{
    const {container}=render(<RowPreview id="t" identity="b" load={loader({duration_seconds:60,bins:[{min:-.5,max:.5},{min:-.4,max:.4}]})}/>);
    await waitFor(()=>expect(container.querySelector(".preview-wave")).not.toBeNull(),{timeout:2000});
    expect(container.querySelector("linearGradient")).toBeNull();
    expect(container.querySelector(".preview-wave")!.getAttribute("style")).toBeNull();
  });
  it("draws the player with the same colours as the row preview for the same track",async()=>{
    const wave=sampleWaveform(120);
    const detail={track:track("a"),source_sha256:"h",pcm_sha256:"p",analysis:null} as unknown as Detail;
    const status:Playback={track_id:"a",duration_seconds:300,position_seconds:10,playing:true,volume:.5,error:null};
    for(const style of ["rgb","blue-green","pink-blue","3band-blueyellow","3band-bluered"] as const){
      act(()=>setWaveformStyle(style));
      const preview=render(<RowPreview id={"t"+style} identity="a" load={loader(wave)}/>);
      await waitFor(()=>expect(preview.container.querySelector("svg")).not.toBeNull(),{timeout:2000});
      const player=render(<PlayerPanel status={status} detail={detail} data={wave} busy={false} onToggle={vi.fn()} onPause={vi.fn()} onSeek={vi.fn()} onVolume={vi.fn()}/>);
      const overview=player.container.querySelector(".wave.overview")!;
      expect(overview).toHaveAttribute("data-wave-style",style);
      expect(preview.container.querySelector(".row-preview-chart")).toHaveAttribute("data-wave-style",style);
      if(style.startsWith("3band")){
        const fills=(root:Element)=>[...root.querySelectorAll(".wave-layer")].map(p=>p.getAttribute("style"));
        expect(fills(overview)).toHaveLength(3);expect(fills(overview)).toEqual(fills(preview.container));
      }else{
        expect(stopColours(overview)).toEqual(stopColours(preview.container));
      }
      preview.unmount();player.unmount();
    }
  });
  it("lets the Appearance page choose the style, remembers it, and shows every option",()=>{
    render(<WaveformSettings/>);
    const options=screen.getAllByRole("radio");
    expect(options).toHaveLength(5);
    expect(screen.getByRole("radio",{name:/^RGB/})).toHaveAttribute("aria-checked","true");
    fireEvent.click(screen.getByRole("radio",{name:/3-band blue and red/}));
    expect(screen.getByRole("radio",{name:/3-band blue and red/})).toHaveAttribute("aria-checked","true");
    expect(localStorage.getItem("cuerithm-waveform-style")).toBe("3band-bluered");
  });
  it("offers Appearance as a settings page without a save button",async()=>{
    const bridge=fakeBridge([]);
    const snapshot=(await bridge.getState()).settings;
    render(<Settings snapshot={snapshot} bridge={bridge} onSaved={vi.fn()} onBack={vi.fn()}/>);
    expect(screen.getByRole("button",{name:/Save settings/})).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Appearance"}));
    expect(screen.getByRole("radiogroup",{name:"Waveform style"})).toBeInTheDocument();
    expect(screen.queryByRole("button",{name:/Save settings/})).not.toBeInTheDocument();
  });
});
