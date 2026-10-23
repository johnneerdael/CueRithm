import {render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import App from "../App";
import {FILE_NOT_FOUND} from "../components/RowPreview";
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

describe("preview text for a missing file and for every other failure",()=>{
  it("keeps the missing-file text for a gone file and the other texts for the other failures",async()=>{
    const bridge=fakeBridge(["gone","unreadable","lookup","blank","fine"].map(track));
    bridge.getTrack=vi.fn(async id=>{
      if(id==="lookup")throw new Error("database is locked");
      return {track:track(id),analysis:null,source_sha256:"hash",pcm_sha256:"pcm",file_available:id!=="gone"};
    });
    bridge.getWaveform=vi.fn(async id=>{
      if(id==="gone"||id==="unreadable")throw new Error("Audio file is missing");
      return id==="fine"?{duration_seconds:240,bins:[{min:-.5,max:.5},{min:-.4,max:.4}]}:{duration_seconds:240,bins:[]};
    });
    render(<App bridge={bridge}/>);
    await screen.findByText("Track gone");
    // The previews settle two at a time; wait until every row has either drawn a chart or shown a dash.
    const preview=(title:string)=>screen.getByText(title).closest("[role=row]")!;
    const settled=()=>["gone","unreadable","lookup","blank","fine"].every(name=>{
      const row=preview("Track "+name);
      return row.querySelector(".row-preview-chart:not(.loading)")!==null||row.querySelector("[role=img][aria-label^='No preview'],[role=img][aria-label^='File not found']")!==null;
    });
    await waitFor(()=>expect(settled()).toBe(true),{timeout:4000});
    const labelOf=(title:string)=>preview(title).querySelector("[role=img][aria-label^='No preview'],[role=img][aria-label^='File not found']")?.getAttribute("aria-label")??null;
    expect(labelOf("Track gone")).toBe(FILE_NOT_FOUND);
    expect(labelOf("Track unreadable")).toMatch(/audio file could not be read/i);
    expect(labelOf("Track lookup")).toBe("No preview: it could not be loaded.");
    expect(labelOf("Track blank")).toBe("No preview: this track has no waveform data.");
    expect(labelOf("Track fine")).toBeNull();
    // No waveform is requested for a file the library knows is gone, and the lookup failure never reaches the waveform step.
    expect(bridge.getWaveform).not.toHaveBeenCalledWith("gone",240);
    expect(bridge.getWaveform).not.toHaveBeenCalledWith("lookup",240);
  });
});
