import {act,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,describe,expect,it,vi} from "vitest";
import PlayerPanel from "../components/PlayerPanel";
import RowPreview from "../components/RowPreview";
import type {Detail,Playback,Waveform} from "../types";
import Settings from "../views/Settings";
import WaveformSettings,{sampleWaveform} from "../views/WaveformSettings";
import {bandColour,contrastRatio,layerFills,legible,palettes,toOklab} from "../waveform-colour";
import type {Theme,WaveformStyle} from "../waveform-colour";
import {buildShape} from "../waveform-shape";
import type {Frame} from "../waveform-shape";
import {setWaveformStyle} from "../waveform-style";
import {fakeBridge,track} from "./bridge";

afterEach(()=>{act(()=>setWaveformStyle("cuerithm"));localStorage.clear();});
const frame=(columns:number,count:number):Frame=>({first:0,last:count,columns,x:bin=>(bin+.5)/count*100,width:100,mid:20,half:15});
// Vertical extent of each "M x,top V bottom" bar.
const heights=(path:string)=>[...path.matchAll(/M[\d.]+,([\d.-]+)V([\d.-]+)/g)].map(m=>Number(m[2])-Number(m[1]));
const stopColours=(root:Element)=>[...root.querySelector("linearGradient")!.querySelectorAll("stop")].map(s=>s.getAttribute("style")!.replace(/\s/g,""));
const loader=(wave:Waveform)=>async()=>({wave,detail:{} as unknown as Detail});
const rgbOf=(css:string)=>css.match(/\d+/g)!.map(Number);
const hex=(h:string)=>[1,3,5].map(i=>parseInt(h.slice(i,i+2),16));
const close=(a:number[],b:number[],by=4)=>a.every((v,i)=>Math.abs(v-b[i])<=by);
const background:Record<Theme,number[]>={light:[240,241,247],dark:[34,38,53]};
const blends=palettes.filter(p=>p.mode==="blend"),layered=palettes.filter(p=>p.mode==="layers");

describe("waveform palettes",()=>{
  it("maps a band that clearly leads to its anchor colour",()=>{
    const [bass,mid,high]=palettes[0].anchors.map(hex);
    expect(close(rgbOf(bandColour("cuerithm",[255,0,0])),bass)).toBe(true);
    expect(close(rgbOf(bandColour("cuerithm",[0,255,0])),mid)).toBe(true);
    expect(close(rgbOf(bandColour("cuerithm",[0,0,255])),high)).toBe(true);
  });
  it("is built on the brand's cobalt and coral",()=>{
    expect(palettes[0].id).toBe("cuerithm");
    expect(palettes[0].anchors.slice(0,2)).toEqual(["#3658F5","#FF785E"]);
  });
  it("leaves silence on the neutral text colour",()=>{
    expect(bandColour("cuerithm",[0,0,0])).toBe("var(--cr-muted)");
  });
  it("steps every palette from dark bass to light highs so bands differ by lightness alone",()=>{
    for(const p of palettes){
      const [bass,mid,high]=p.anchors.map(a=>toOklab(hex(a))[0]);
      expect(mid-bass,p.id).toBeGreaterThanOrEqual(.1);
      expect(high-mid,p.id).toBeGreaterThanOrEqual(.1);
    }
  });
  it("blends between anchors in lightness and keeps the blend saturated instead of greying out",()=>{
    const [bass,mid]=palettes[0].anchors.map(a=>toOklab(hex(a)));
    const mixed=toOklab(rgbOf(bandColour("cuerithm",[255,255,0])));
    expect(mixed[0]).toBeGreaterThan(bass[0]);expect(mixed[0]).toBeLessThan(mid[0]);
    const chroma=(l:number[])=>Math.hypot(l[1],l[2]);
    // Cobalt and coral are near-opposite hues, so a plain average would sit close to grey.
    expect(chroma(mixed)).toBeGreaterThan(Math.min(chroma(bass),chroma(mid))*.7);
  });
  it("keeps every blended colour at 3:1 contrast or better on both themes",()=>{
    const grid=[0,85,170,255],triples=grid.flatMap(r=>grid.flatMap(g=>grid.map(b=>[r,g,b] as [number,number,number]))).filter(t=>t[0]+t[1]+t[2]>0);
    for(const theme of ["light","dark"] as const)for(const p of blends)for(const t of triples){
      expect(contrastRatio(rgbOf(bandColour(p.id,t,theme)),background[theme]),`${p.id} ${theme} ${t}`).toBeGreaterThanOrEqual(2.95);
    }
  });
  it("keeps the layer colours legible on both themes",()=>{
    for(const theme of ["light","dark"] as const)for(const p of layered)for(const fill of layerFills(p.id,theme)){
      expect(contrastRatio(rgbOf(fill),background[theme]),`${p.id} ${theme}`).toBeGreaterThanOrEqual(2.95);
    }
  });
  it("lifts a colour that is too dark for the dark theme and darkens one that is too pale for the light theme",()=>{
    const cobalt=hex("#3658F5"),ice=hex("#9AF0DC");
    expect(contrastRatio(cobalt,background.dark)).toBeLessThan(3);
    expect(toOklab(legible(cobalt,"dark"))[0]).toBeGreaterThan(toOklab(cobalt)[0]);
    expect(contrastRatio(ice,background.light)).toBeLessThan(3);
    const darker=legible(ice,"light");
    expect(toOklab(darker)[0]).toBeLessThan(toOklab(ice)[0]);
    expect(contrastRatio(darker,background.light)).toBeGreaterThanOrEqual(2.95);
  });
  it("leaves colours that are already legible exactly as designed",()=>{
    expect(legible([255,120,94],"dark")).toEqual([255,120,94]);
    expect(legible([41,70,216],"light")).toEqual([41,70,216]);
  });
});

describe("waveform shape",()=>{
  const wave=(rms:number[],peak:number,bands:[number,number,number]=[255,0,0]):Waveform=>({duration_seconds:60,bins:rms.map(()=>({min:-peak,max:peak})),rms,bands:rms.map(()=>bands),levels:rms.map(r=>[r,r/2,r/8])});
  it("sizes bars by average level, not by peak",()=>{
    // Identical peaks, different loudness: only the average level may change the bar.
    const shape=buildShape(wave([.1,.1,.4,.4],.9),"cuerithm",frame(4,4));
    if(shape.mode!=="bars")throw new Error("expected bars");
    const [a,b,c,d]=heights(shape.path);
    expect(a).toBeCloseTo(b);expect(c).toBeCloseTo(d);
    expect(c).toBeGreaterThan(a*2);
  });
  it("scales each track so a quiet one still fills the height",()=>{
    const quiet=buildShape(wave([.02,.04,.08,.1],.9),"cuerithm",frame(4,4)),loud=buildShape(wave([.2,.4,.8,1],.9),"cuerithm",frame(4,4));
    if(quiet.mode!=="bars"||loud.mode!=="bars")throw new Error("expected bars");
    const q=heights(quiet.path),l=heights(loud.path);
    expect(Math.max(...q)).toBeCloseTo(Math.max(...l),1);
    expect(Math.max(...q)).toBeGreaterThan(20);
  });
  it("falls back to the peak envelope when a waveform has no level data",()=>{
    const old:Waveform={duration_seconds:60,bins:[{min:-.2,max:.2},{min:-.8,max:.8}]};
    const shape=buildShape(old,"cuerithm",frame(2,2));
    if(shape.mode!=="bars")throw new Error("expected bars");
    const [small,large]=heights(shape.path);
    expect(large).toBeGreaterThan(small*3);
    expect(shape.stops).toEqual([]);
  });
  it("gives one gradient stop per column, coloured by the chosen palette",()=>{
    const w=wave([.2,.2,.2,.2],.5),a=buildShape(w,"cuerithm",frame(4,4)),b=buildShape(w,"dusk",frame(4,4));
    if(a.mode!=="bars"||b.mode!=="bars")throw new Error("expected bars");
    expect(a.stops).toHaveLength(4);
    expect(a.stops[0].color).toBe(bandColour("cuerithm",[255,0,0]));
    expect(b.stops[0].color).toBe(bandColour("dusk",[255,0,0]));
    expect(a.stops.map(s=>s.offset)).toEqual([...a.stops.map(s=>s.offset)].sort((x,y)=>x-y));
  });
  it("judges each band against its own typical level in the track, so quiet highs still show",()=>{
    // Highs never exceed 100/255 here; relative to this track they are at full strength.
    const shape=buildShape(wave([.2,.2,.2,.2],.5,[255,0,100]),"cuerithm",frame(4,4));
    if(shape.mode!=="bars")throw new Error("expected bars");
    expect(shape.stops[0].color).toBe(bandColour("cuerithm",[255,0,255]));
    // A floor stops a band that is essentially absent from being boosted into view.
    const absent=buildShape(wave([.2,.2,.2,.2],.5,[255,0,3]),"cuerithm",frame(4,4));
    if(absent.mode!=="bars")throw new Error("expected bars");
    expect(absent.stops[0].color).toBe(bandColour("cuerithm",[255,0,16]));
  });
  it("groups bins into at most the requested columns",()=>{
    const shape=buildShape(wave(Array(1000).fill(.3),.5),"cuerithm",frame(50,1000));
    if(shape.mode!=="bars")throw new Error("expected bars");
    expect(heights(shape.path)).toHaveLength(50);expect(shape.stops).toHaveLength(50);
  });
  const top=(path:string)=>Number(path.match(/^M[\d.]+,([\d.-]+)/)![1]);
  const lowest=(path:string)=>Math.max(...[...path.matchAll(/,([\d.-]+)/g)].map(m=>Number(m[1])));
  it("overlays the layers around the centre for zoomed views, bass widest and the high layer a thin core",()=>{
    const shape=buildShape(wave([.3,.3,.3,.3],.6),"layers",frame(4,4));
    if(shape.mode!=="layers")throw new Error("expected layers");
    expect(shape.layers.map(l=>l.fill)).toEqual(layerFills("layers"));
    // Smaller y is further above the centre line (20): bass reaches furthest, then mids, then highs.
    const [bass,mids,highs]=shape.layers.map(l=>20-top(l.path));
    expect(bass).toBeGreaterThan(mids);expect(mids).toBeGreaterThan(highs);
    expect(highs).toBeLessThan(bass/5);
    expect(highs).toBeGreaterThan(0);
  });
  // Top edge of the outermost layer at each column (the layer paths list their top edge first).
  const silhouette=(layers:{path:string}[])=>{
    const tops=layers.map(l=>l.path.slice(1,-1).split("L").slice(0,l.path.split("L").length/2).map(p=>Number(p.split(",")[1])));
    return tops[0].map((_,i)=>Math.min(...tops.map(t=>t[i])));
  };
  it("makes the zoomed layers as tall as the bar styles' bars, so a track looks as full in every style",()=>{
    const w=wave([.1,.2,.3,.4],.9),bars=buildShape(w,"cuerithm",frame(4,4)),layers=buildShape(w,"layers",frame(4,4));
    if(bars.mode!=="bars"||layers.mode!=="layers")throw new Error("unexpected modes");
    const barTops=[...bars.path.matchAll(/M[\d.]+,([\d.-]+)V/g)].map(m=>Number(m[1]));
    silhouette(layers.layers).forEach((y,i)=>expect(y).toBeCloseTo(barTops[i],0));
  });
  it("keeps a sidechained track full between kicks instead of collapsing to a sliver",()=>{
    // Same loudness in both columns; the bass only exists on the kick (first column).
    const kick:Waveform={...wave([.3,.3],.6),levels:[[.6,.1,.02],[.03,.1,.02]]};
    const shape=buildShape(kick,"layers",frame(2,2));
    if(shape.mode!=="layers")throw new Error("expected layers");
    const [onKick,between]=silhouette(shape.layers);
    expect(between).toBeCloseTo(onKick,0);
    expect(20-between).toBeGreaterThan(8);
    // The colours still say what changed: bass leads on the kick, mids lead between kicks.
    const bassHeight=shape.layers[0].path.split("L").slice(0,2).map(p=>20-Number(p.split(",")[1].replace("M","")));
    expect(bassHeight[0]).toBeGreaterThan(bassHeight[1]*1.5);
  });
  it("stacks the layers up from the bottom edge for overviews: bass, then mids, then highs",()=>{
    const shape=buildShape(wave([.3,.3,.3,.3],.6),"layers-tonal",{...frame(4,4),stacked:true});
    if(shape.mode!=="layers")throw new Error("expected layers");
    expect(shape.layers.map(l=>l.fill)).toEqual(layerFills("layers-tonal"));
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
    const shape=buildShape(loud,"layers",{...frame(2,2),stacked:true});
    if(shape.mode!=="layers")throw new Error("expected layers");
    const height=30,[bass,mids]=shape.layers.map(l=>top(l.path));
    expect((35-bass)/height).toBeCloseTo(.5,1);
    expect((bass-mids)/height).toBeCloseTo(.3,1);
  });
  it("applies the theme through buildShape and leaves colours as designed without one",()=>{
    const w=wave([.3,.3],.5,[255,255,0]);
    const light=buildShape(w,"cuerithm",{...frame(2,2),theme:"light"}),plain=buildShape(w,"cuerithm",frame(2,2));
    if(light.mode!=="bars"||plain.mode!=="bars")throw new Error("expected bars");
    expect(light.stops[0].color).toBe(bandColour("cuerithm",[255,255,0],"light"));
    expect(plain.stops[0].color).toBe(bandColour("cuerithm",[255,255,0]));
  });
  it("draws a layered style as bars with that palette's colours when levels are missing",()=>{
    const old:Waveform={duration_seconds:60,bins:[{min:-.5,max:.5},{min:-.5,max:.5}],rms:[.3,.3],bands:[[0,255,0],[0,255,0]]};
    const shape=buildShape(old,"layers",frame(2,2));
    expect(shape.mode).toBe("bars");
  });
});

describe("waveform styles in the app",()=>{
  it("strokes a row preview with the chosen palette and switches live, including to a layered style",async()=>{
    const {container}=render(<RowPreview id="t" identity="a" load={loader(sampleWaveform(120))}/>);
    await waitFor(()=>expect(container.querySelector("linearGradient")).not.toBeNull(),{timeout:2000});
    const first=stopColours(container);
    expect(container.querySelector(".preview-wave")!.getAttribute("style")).toContain("url(#"+container.querySelector("linearGradient")!.id+")");
    act(()=>setWaveformStyle("dusk"));
    expect(container.querySelector(".row-preview-chart")).toHaveAttribute("data-wave-style","dusk");
    expect(stopColours(container)).not.toEqual(first);
    act(()=>setWaveformStyle("layers"));
    expect(container.querySelector("linearGradient")).toBeNull();
    expect(container.querySelectorAll(".wave-layer")).toHaveLength(3);
  });
  it("keeps the single accent stroke for a waveform without band data",async()=>{
    const {container}=render(<RowPreview id="t" identity="b" load={loader({duration_seconds:60,bins:[{min:-.5,max:.5},{min:-.4,max:.4}]})}/>);
    await waitFor(()=>expect(container.querySelector(".preview-wave")).not.toBeNull(),{timeout:2000});
    expect(container.querySelector("linearGradient")).toBeNull();
    expect(container.querySelector(".preview-wave")!.getAttribute("style")).toBeNull();
  });
  it("draws the player with the same colours as the row preview for the same track, in every style and theme",async()=>{
    const wave=sampleWaveform(120);
    const detail={track:track("a"),source_sha256:"h",pcm_sha256:"p",analysis:null} as unknown as Detail;
    const status:Playback={track_id:"a",duration_seconds:300,position_seconds:10,playing:true,volume:.5,error:null};
    for(const theme of ["dark","light"] as const){
      document.documentElement.dataset.theme=theme;
      for(const {id:style} of palettes){
        act(()=>setWaveformStyle(style));
        const preview=render(<RowPreview id={"t"+style+theme} identity="a" load={loader(wave)}/>);
        await waitFor(()=>expect(preview.container.querySelector("svg")).not.toBeNull(),{timeout:2000});
        const player=render(<PlayerPanel status={status} detail={detail} data={wave} busy={false} onToggle={vi.fn()} onPause={vi.fn()} onSeek={vi.fn()} onVolume={vi.fn()}/>);
        const overview=player.container.querySelector(".wave.overview")!;
        expect(overview).toHaveAttribute("data-wave-style",style);
        expect(preview.container.querySelector(".row-preview-chart")).toHaveAttribute("data-wave-style",style);
        if(style.startsWith("layers")){
          const fills=(root:Element)=>[...root.querySelectorAll(".wave-layer")].map(p=>p.getAttribute("style"));
          expect(fills(overview)).toHaveLength(3);expect(fills(overview)).toEqual(fills(preview.container));
        }else{
          expect(stopColours(overview),`${style} ${theme}`).toEqual(stopColours(preview.container));
        }
        preview.unmount();player.unmount();
      }
    }
    delete document.documentElement.dataset.theme;
  });
  it("recolours for the active theme when the theme changes",async()=>{
    const wave=sampleWaveform(120);
    document.documentElement.dataset.theme="dark";
    const {container}=render(<RowPreview id="t" identity="theme" load={loader(wave)}/>);
    await waitFor(()=>expect(container.querySelector("linearGradient")).not.toBeNull(),{timeout:2000});
    const dark=stopColours(container);
    act(()=>{document.documentElement.dataset.theme="light";});
    await waitFor(()=>expect(stopColours(container)).not.toEqual(dark));
    delete document.documentElement.dataset.theme;
  });
  it("lets the Appearance page choose the style, remembers it, and shows every option",()=>{
    render(<WaveformSettings/>);
    expect(screen.getAllByRole("radio")).toHaveLength(palettes.length);
    expect(screen.getByRole("radio",{name:/^CueRithm/})).toHaveAttribute("aria-checked","true");
    fireEvent.click(screen.getByRole("radio",{name:/^Layers, tonal/}));
    expect(screen.getByRole("radio",{name:/^Layers, tonal/})).toHaveAttribute("aria-checked","true");
    expect(localStorage.getItem("cuerithm-waveform-style")).toBe("layers-tonal" satisfies WaveformStyle);
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
