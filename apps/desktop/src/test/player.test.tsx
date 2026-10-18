import {render,screen,within} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import PlayerPanel from "../components/PlayerPanel";
import {track} from "./bridge";
import type {Detail,Playback} from "../types";

it("keeps the entire track and every cue visible while playback moves",()=>{
  const detail:Detail={track:track("a"),source_sha256:"hash",pcm_sha256:"pcm",analysis:{complete:true,source_sample_rate:44100,analysis:{duration_seconds:360,key:null,energy:null,tempo:{bpm:120,beats_seconds:[0,.5,1],downbeats_seconds:null},lexicon_cues:{cues:[{time_seconds:0,name:"Start",color:"Red"},{time_seconds:180,name:"Breakdown",color:"Blue"},{time_seconds:359,name:"Last beat",color:"Yellow"}],emergency_loop:null}}}};
  const status:Playback={track_id:"a",duration_seconds:360,position_seconds:7,playing:true,volume:.1,error:null};
  const props={detail,status,data:{duration_seconds:360,bins:[{min:-1,max:1}]},busy:false,onToggle:vi.fn(),onPause:vi.fn(),onSeek:vi.fn(),onVolume:vi.fn()};
  const {rerender}=render(<PlayerPanel {...props}/>);
  expect(screen.getByRole("combobox",{name:"Waveform zoom"})).toHaveValue("0");
  expect(screen.getByRole("slider",{name:"Waveform seek"}).querySelectorAll("[data-cue]")).toHaveLength(3);
  rerender(<PlayerPanel {...props} status={{...status,position_seconds:240}}/>);
  expect(screen.getByRole("slider",{name:"Waveform seek"}).querySelectorAll("[data-cue]")).toHaveLength(3);
  expect(within(screen.getByRole("region",{name:"Music player"})).getByText("6:00.0")).toBeInTheDocument();
});
