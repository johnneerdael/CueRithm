import {render,screen} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";

const audio=/audio file could not be read/i,request="No preview: it could not be loaded.";
async function preview(setup:(bridge:ReturnType<typeof fakeBridge>)=>void){
  const bridge=fakeBridge([track("a")]);
  setup(bridge);
  render(<App bridge={bridge}/>);
  await screen.findByText("Track a");
  return screen.findByRole("img",{name:/^No preview/},{timeout:3000});
}

describe("preview loader in the track table",()=>{
  it("blames the audio when the waveform fails after the track lookup worked",async()=>{
    const dash=await preview(bridge=>{bridge.getWaveform=vi.fn(async()=>{throw new Error("Audio file is missing");});});
    expect(dash.getAttribute("aria-label")).toMatch(audio);
  });
  it("calls it a failed request when the track lookup fails",async()=>{
    const dash=await preview(bridge=>{bridge.getTrack=vi.fn(async()=>{throw new Error("database is locked");});});
    expect(dash).toHaveAttribute("aria-label",request);
  });
  it("still calls it a failed request when the lookup and the waveform both fail",async()=>{
    const dash=await preview(bridge=>{
      bridge.getTrack=vi.fn(async()=>{throw new Error("database is locked");});
      bridge.getWaveform=vi.fn(async()=>{throw new Error("Audio file is missing");});
    });
    expect(dash).toHaveAttribute("aria-label",request);
  });
  it("says there is no waveform data when both calls work but the waveform is empty",async()=>{
    const dash=await preview(()=>{});
    expect(dash).toHaveAttribute("aria-label","No preview: this track has no waveform data.");
  });
});
