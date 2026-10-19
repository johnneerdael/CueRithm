import {render,waitFor} from "@testing-library/react";
import {describe,expect,it} from "vitest";
import RowPreview from "../components/RowPreview";
import type {Detail,Waveform} from "../types";
import {bandColour,gradientStops} from "../waveform-colour";

const wave=(bands?:Waveform["bands"]):Waveform=>({duration_seconds:60,bins:(bands??[[0,0,0],[0,0,0]]).map(()=>({min:-.5,max:.5})),bands});

describe("waveform colour",()=>{
  it("maps bass, mids and highs to red, green and blue and leaves silence neutral",()=>{
    expect(bandColour([255,0,0])).toBe("rgb(255,0,0)");
    expect(bandColour([255,255,0])).toBe("rgb(255,255,0)");
    expect(bandColour([0,0,0])).toBe("var(--cr-muted)");
  });
  it("averages groups of bins into at most the requested number of stops",()=>{
    const bands:[number,number,number][]=[[255,0,0],[255,0,0],[0,255,0],[0,255,0]];
    const stops=gradientStops({bands},0,4,i=>(i+.5)/4,2);
    expect(stops).toEqual([{offset:.25,color:"rgb(255,0,0)"},{offset:.75,color:"rgb(0,255,0)"}]);
    expect(gradientStops({bands},0,4,i=>(i+.5)/4,4)).toHaveLength(4);
  });
  it("has no stops without band data, so callers keep the single accent colour",()=>{
    expect(gradientStops({},0,4,()=>0)).toEqual([]);
    expect(gradientStops({bands:[]},0,4,()=>0)).toEqual([]);
  });
  it("clamps stop offsets into the gradient range",()=>{
    const stops=gradientStops({bands:[[1,1,1],[1,1,1]]},0,2,i=>i-5);
    expect(stops.every(s=>s.offset>=0&&s.offset<=1)).toBe(true);
  });
  it("strokes a row preview with a per-column gradient when the waveform has bands",async()=>{
    const bands:[number,number,number][]=Array.from({length:240},(_,i)=>i<120?[255,40,0]:[0,200,255]);
    const {container}=render(<RowPreview id="t" identity="a" load={async()=>({wave:wave(bands),detail:{} as unknown as Detail})}/>);
    await waitFor(()=>expect(container.querySelector("linearGradient")).not.toBeNull(),{timeout:2000});
    const gradient=container.querySelector("linearGradient")!;
    const stops=[...gradient.querySelectorAll("stop")];
    expect(stops).toHaveLength(120);
    expect(stops[0].getAttribute("style")).toContain("rgb(255, 40, 0)");
    expect(stops[119].getAttribute("style")).toContain("rgb(0, 200, 255)");
    expect(container.querySelector(".preview-wave")!.getAttribute("style")).toContain("url(#"+gradient.id+")");
  });
  it("keeps the single accent stroke for a cache without bands",async()=>{
    const {container}=render(<RowPreview id="t" identity="b" load={async()=>({wave:wave(undefined),detail:{} as unknown as Detail})}/>);
    await waitFor(()=>expect(container.querySelector(".preview-wave")).not.toBeNull(),{timeout:2000});
    expect(container.querySelector("linearGradient")).toBeNull();
    expect(container.querySelector(".preview-wave")!.getAttribute("style")).toBeNull();
  });
});
