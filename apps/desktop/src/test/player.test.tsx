import {render,screen,within,fireEvent} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import PlayerPanel from "../components/PlayerPanel";
import {track} from "./bridge";
import type {Detail,Playback} from "../types";

it("defaults to 32 seconds while keeping the overview complete and toggles the main waveform",()=>{
  const detail:Detail={track:track("a"),source_sha256:"hash",pcm_sha256:"pcm",analysis:{complete:true,source_sample_rate:44100,analysis:{duration_seconds:360,key:null,energy:null,tempo:{bpm:120,beats_seconds:[0,.5,1],downbeats_seconds:null},lexicon_cues:{cues:[{time_seconds:0,name:"Start",color:"Red"},{time_seconds:180,name:"Breakdown",color:"Blue"},{time_seconds:359,name:"Last beat",color:"Yellow"}],emergency_loop:null}}}};
  const status:Playback={track_id:"a",duration_seconds:360,position_seconds:7,playing:true,volume:.1,error:null};
  const props={detail,status,data:{duration_seconds:360,bins:[{min:-1,max:1}]},busy:false,onToggle:vi.fn(),onPause:vi.fn(),onSeek:vi.fn(),onVolume:vi.fn()};
  const {rerender}=render(<PlayerPanel {...props}/>);
  expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  const overview=screen.getByRole("slider",{name:"Track overview"});
  const zoomed=screen.getByRole("button",{name:"Waveform zoom: 32 seconds"});
  const axis=within(zoomed.nextElementSibling as HTMLElement);
  expect(zoomed).toHaveAttribute("aria-pressed","true");
  expect(zoomed.querySelectorAll("[data-cue]")).toHaveLength(1);
  expect(axis.getByText("0:32.0")).toBeInTheDocument();
  expect(overview).toHaveAttribute("aria-valuemax","360");
  expect(overview.querySelectorAll("[data-cue]")).toHaveLength(3);
  rerender(<PlayerPanel {...props} status={{...status,position_seconds:240}}/>);
  expect(zoomed.querySelectorAll("[data-cue]")).toHaveLength(0);
  expect(axis.getByText("3:48.8")).toBeInTheDocument();
  expect(axis.getByText("4:20.8")).toBeInTheDocument();
  expect(overview.querySelectorAll("[data-cue]")).toHaveLength(3);
  fireEvent.click(zoomed);
  const entire=screen.getByRole("button",{name:"Waveform zoom: Entire track"});
  expect(entire).toHaveAttribute("aria-pressed","false");
  expect(entire.querySelectorAll("[data-cue]")).toHaveLength(3);
  expect(axis.getByText("6:00.0")).toBeInTheDocument();
  fireEvent.keyDown(entire,{key:"Enter"});
  expect(screen.getByRole("button",{name:"Waveform zoom: 32 seconds"})).toBeInTheDocument();
  fireEvent.keyDown(zoomed,{key:" "});
  expect(screen.getByRole("button",{name:"Waveform zoom: Entire track"})).toBeInTheDocument();
  fireEvent.click(entire);
  expect(screen.getByRole("button",{name:"Waveform zoom: 32 seconds"})).toBeInTheDocument();
  rerender(<PlayerPanel {...props} status={{...status,position_seconds:359}}/>);
  expect(axis.getByText("5:28.0")).toBeInTheDocument();
  expect(axis.getByText("6:00.0")).toBeInTheDocument();
  rerender(<PlayerPanel {...props} status={{...status,duration_seconds:10,position_seconds:0}} data={{duration_seconds:10,bins:[{min:-1,max:1}]}}/>);
  expect(axis.getByText("0:00.0")).toBeInTheDocument();
  expect(axis.getByText("0:10.0")).toBeInTheDocument();
  expect(props.onSeek).not.toHaveBeenCalled();
  expect(props.onToggle).not.toHaveBeenCalled();
});

it("uses published cue names, colors, positions and grid while retaining raw analysis",()=>{
  const detail:Detail={track:track("a"),source_sha256:"hash",pcm_sha256:"pcm",analysis:{complete:true,source_sample_rate:44100,analysis:{duration_seconds:160,key:null,energy:null,tempo:{bpm:128.345,beats_seconds:[0,.5,1],downbeats_seconds:null},lexicon_cues:{cues:[{time_seconds:0,name:"Raw marker",color:"Red"}],emergency_loop:null}}}};
  Object.assign(detail.analysis!,{publication:{grid:{bpm:128.35,startTime:.125},cues:[{name:"Start",startTime:.125,type:"1",position:0,color:"yellow"},{name:"End / Fade-out",startTime:159.75,type:"1",position:5,color:"violet"}],implementation_sha256:"fixture",inputs_sha256:"fixture"}});
  const onSeek=vi.fn();render(<PlayerPanel detail={detail} status={{track_id:"a",duration_seconds:160,position_seconds:0,playing:true,volume:.1,error:null}} data={{duration_seconds:160,bins:[]}} busy={false} onToggle={vi.fn()} onPause={vi.fn()} onSeek={onSeek} onVolume={vi.fn()}/>);
  expect(screen.queryByRole("button",{name:/Raw marker/})).not.toBeInTheDocument();
  const end=screen.getByRole("button",{name:"F End / Fade-out 2:39.8"});expect(end).toHaveStyle({"--cue":"#aa3bff"});
  fireEvent.click(end);expect(onSeek).toHaveBeenCalledWith(159.75);
  fireEvent.click(screen.getByRole("button",{name:"Waveform zoom: 32 seconds"}));
  const wave=screen.getByRole("button",{name:"Waveform zoom: Entire track"});expect(wave.querySelectorAll(".beat-line").length).toBeGreaterThan(300);
  expect(detail.analysis!.analysis.tempo!.bpm).toBe(128.345);
});

it("shows imported cues and variable tempo without inventing DSP or an unknown beat phase",()=>{
 const detail:Detail={track:{...track("imported"),key:"8A",bpm:120,energy:7},source_sha256:"",pcm_sha256:null,analysis:null,musical:{id:"musical",source:"lexicon",cues:[{id:"negative",kind:"cue",start_seconds:-1.25,end_seconds:null,slot:null,memory:true,name:"Before start",color:null,active_loop:false,source_data:{}},{id:"loop",kind:"loop",start_seconds:10,end_seconds:14,slot:2,memory:false,name:"Saved loop",color:null,active_loop:true,source_data:{}}],tempo_markers:[{id:"one",start_seconds:0,bpm:120,beat_number:null,beats_per_bar:null,source_data:{}},{id:"two",start_seconds:20,bpm:128,beat_number:null,beats_per_bar:null,source_data:{}}]}};
 const onSeek=vi.fn();render(<PlayerPanel detail={detail} status={{track_id:"imported",duration_seconds:60,position_seconds:25,playing:false,volume:.1,error:null}} data={{duration_seconds:60,bins:[]}} busy={false} onToggle={vi.fn()} onPause={vi.fn()} onSeek={onSeek} onVolume={vi.fn()}/>);
 expect(screen.getByText("SAVED CUE TIMING")).toBeVisible();expect(screen.getByText(/Variable tempo · Beat phase unknown/)).toBeVisible();expect(screen.getByText("128.00")).toBeVisible();expect(screen.getByText("8A")).toBeVisible();expect(document.querySelectorAll(".beat-line")).toHaveLength(0);
 fireEvent.click(screen.getByRole("button",{name:/M Before start/}));expect(onSeek).toHaveBeenLastCalledWith(0);expect(screen.getByText("−0:01.3")).toBeVisible();expect(detail.musical!.cues[0].start_seconds).toBe(-1.25);expect(screen.getByRole("button",{name:/Saved loop/})).toHaveAttribute("title","Seek to Saved loop · Loop ends at 14s");expect(detail.analysis).toBeNull();
});

it("retains real native analysis beats when accepted data is its saved-publication projection",async()=>{const {playbackBeats,playbackBpm}=await import("../published-analysis");const analysis={complete:true,source_sample_rate:44100,analysis:{duration_seconds:20,key:null,energy:null,tempo:{bpm:120,beats_seconds:[0,.5,1],downbeats_seconds:null},lexicon_cues:null}};const musical={id:"native-snapshot",source:"cuerithm_saved_publication",cues:[],tempo_markers:[{id:"anchor",start_seconds:.25,bpm:120,beat_number:null,beats_per_bar:null,source_data:{}}]};expect(playbackBeats(analysis,20,musical)).toEqual([0,.5,1]);expect(playbackBpm(analysis,musical,0)).toBe(120);});
