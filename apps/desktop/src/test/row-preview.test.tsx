import {render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it} from "vitest";
import RowPreview from "../components/RowPreview";
import type {Analysis,Detail,PublishedCue,Waveform} from "../types";
import {sampleWaveform} from "../views/WaveformSettings";

const analysis=(publication?:Analysis["publication"]):Analysis=>({
  publication,source_sample_rate:44100,complete:true,
  analysis:{duration_seconds:60,key:null,tempo:{bpm:128,beats_seconds:[],downbeats_seconds:null},energy:null,lexicon_cues:{cues:[{time_seconds:10,name:"Detector drop",color:"Red"}],emergency_loop:null}},
});
const saved=(...cues:PublishedCue[]):Analysis["publication"]=>({grid:{bpm:128,startTime:0},cues,implementation_sha256:"impl",inputs_sha256:"inputs"});
const loader=(detail:Analysis|null,wave:Waveform=sampleWaveform(40))=>async()=>({wave,detail:{analysis:detail} as unknown as Detail});
const markers=(root:Element)=>[...root.querySelectorAll("[data-cue]")].map(node=>node.getAttribute("data-cue"));

describe("row preview markers",()=>{
  it("draws the detector's cues until the cues are corrected",async()=>{
    const {container}=render(<RowPreview id="detected" identity="a" load={loader(analysis())}/>);
    await waitFor(()=>expect(markers(container)).toEqual(["Detector drop"]),{timeout:2000});
    expect(screen.getByRole("img",{name:"1 cue"})).toHaveAttribute("title","A · Detector drop");
  });
  it("draws the saved cues instead of the detector's once they exist",async()=>{
    const publication=saved({name:"Saved drop",startTime:30,type:"1",position:0,color:"red"},{name:"Saved loop",startTime:40,endTime:44,type:"5",position:1,color:"blue"});
    const {container}=render(<RowPreview id="corrected" identity="a" load={loader(analysis(publication))}/>);
    await waitFor(()=>expect(markers(container)).toEqual(["Saved drop","Saved loop"]),{timeout:2000});
    expect(screen.getByRole("img",{name:"2 cues"})).toHaveAttribute("title","A · Saved drop\nB · Saved loop");
  });
  it("letters a saved cue by its slot, as the player does, when an earlier slot was removed",async()=>{
    const publication=saved({name:"First",startTime:5,type:"1",position:0,color:"red"},{name:"Third",startTime:30,type:"1",position:2,color:"red"});
    render(<RowPreview id="gap" identity="a" load={loader(analysis(publication))}/>);
    expect(await screen.findByRole("img",{name:"2 cues"},{timeout:2000})).toHaveAttribute("title","A · First\nC · Third");
  });
});

describe("row preview dash",()=>{
  it("says the audio could not be read when the preview fails to load",async()=>{
    const failing=async()=>{throw new Error("Audio file is missing");};
    const {container}=render(<RowPreview id="gone" identity="a" load={failing}/>);
    const dash=await screen.findByRole("img",{name:/audio file could not be read/i},{timeout:2000});
    expect(dash).toHaveTextContent("—");
    expect(dash).toHaveAttribute("title",dash.getAttribute("aria-label")!);
    expect(container.querySelector("svg")).toBeNull();
  });
  it("says there is no waveform when the track decodes to nothing",async()=>{
    render(<RowPreview id="empty" identity="a" load={loader(analysis(),{duration_seconds:0,bins:[]})}/>);
    const dash=await screen.findByRole("img",{name:/no waveform data/i},{timeout:2000});
    expect(dash).toHaveTextContent("—");
  });
  it("still explains the dash when no preview loader is available",()=>{
    render(<RowPreview id="none" identity="a"/>);
    expect(screen.getByRole("img",{name:"No preview available."})).toHaveTextContent("—");
  });
});
