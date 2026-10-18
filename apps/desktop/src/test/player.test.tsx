import {render,screen,within,fireEvent} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import PlayerPanel from "../components/PlayerPanel";
import {track} from "./bridge";
import type {Detail,Playback} from "../types";

it("keeps the entire track and every cue visible while playback moves",()=>{
  const detail:Detail={track:track("a"),source_sha256:"hash",pcm_sha256:"pcm",analysis:{complete:true,source_sample_rate:44100,analysis:{duration_seconds:360,key:null,energy:null,tempo:{bpm:120,beats_seconds:[0,.5,1],downbeats_seconds:null},lexicon_cues:{cues:[{time_seconds:0,name:"Start",color:"Red"},{time_seconds:180,name:"Breakdown",color:"Blue"},{time_seconds:359,name:"Last beat",color:"Yellow"}],emergency_loop:null}}}};
  const status:Playback={track_id:"a",duration_seconds:360,position_seconds:7,playing:true,volume:.1,error:null};
  const props={detail,status,data:{duration_seconds:360,bins:[{min:-1,max:1}]},busy:false,onToggle:vi.fn(),onPause:vi.fn(),onSeek:vi.fn(),onVolume:vi.fn()};
  const {rerender}=render(<PlayerPanel {...props}/>);
  expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Waveform zoom: Entire track"})).toHaveAttribute("aria-pressed","false");
  expect(screen.getByRole("button",{name:"Waveform zoom: Entire track"}).querySelectorAll("[data-cue]")).toHaveLength(3);
  rerender(<PlayerPanel {...props} status={{...status,position_seconds:240}}/>);
  expect(screen.getByRole("button",{name:"Waveform zoom: Entire track"}).querySelectorAll("[data-cue]")).toHaveLength(3);
  expect(within(screen.getByRole("region",{name:"Music player"})).getByText("6:00.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Waveform zoom: Entire track"}));
  const zoomed=screen.getByRole("button",{name:"Waveform zoom: 16 seconds"});
  expect(zoomed).toHaveAttribute("aria-pressed","true");
  expect(zoomed.querySelectorAll("[data-cue]")).toHaveLength(0);
  expect(props.onSeek).not.toHaveBeenCalled();
  fireEvent.keyDown(zoomed,{key:"Enter"});
  expect(screen.getByRole("button",{name:"Waveform zoom: Entire track"}).querySelectorAll("[data-cue]")).toHaveLength(3);
  fireEvent.keyDown(screen.getByRole("button",{name:"Waveform zoom: Entire track"}),{key:" "});
  expect(screen.getByRole("button",{name:"Waveform zoom: 16 seconds"})).toBeInTheDocument();
  expect(props.onToggle).not.toHaveBeenCalled();
});
