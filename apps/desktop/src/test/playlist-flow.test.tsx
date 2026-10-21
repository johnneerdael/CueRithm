import {fireEvent,render,screen,within} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {describe,expect,it,vi} from "vitest";
import PlaylistFlow from "../components/PlaylistFlow";
import {buildPlaylistFlow,compareFlowKeys} from "../playlist-flow";
import type {Track} from "../types";

const track=(id:string,patch:Partial<Track>={}):Track=>({id,title:"Track "+id,artist:"Artist",album:"",genre:"",metadata:{},source_sha256:id,display_path:"fixture.wav",state:"ready",archived:false,key:"8A",bpm:120,energy:5,duration:240,...patch});
const noop=()=>{};

describe("playlist flow",()=>{
 it("uses canonical membership order regardless of table sorting and reports BPM changes",()=>{
  const rows=buildPlaylistFlow([track("b",{bpm:122}),track("a")],["a","b"]);
  expect(rows.map(row=>row.track.id)).toEqual(["a","b"]);
  expect(rows[1].bpmChange).toBe(2);
  render(<PlaylistFlow tracks={[track("b",{bpm:122}),track("a")]} orderedTrackIds={["a","b"]} onSelect={noop} onPlay={noop}/>);
  const items=screen.getAllByRole("listitem");
  expect(within(items[0]).getByRole("button",{name:"Select Track a"})).toBeInTheDocument();
  expect(within(items[1]).getByText("+2 BPM")).toBeInTheDocument();
 });

 it("keeps missing measurements unknown and never invents adjacency across unloaded tracks",()=>{
  const tracks=[track("a",{energy:null,key:null,bpm:null}),track("b",{key:"9A"}),track("d",{energy:Number.NaN,key:"?",bpm:0})];
  const rows=buildPlaylistFlow(tracks,["a","b","c","d"]);
  expect(rows[1]).toMatchObject({bpmChange:null,keyComparison:"unknown"});
  expect(rows[2]).toMatchObject({position:4,energy:null,bpm:null,predecessor:"unloaded",keyComparison:"unknown"});
  render(<PlaylistFlow tracks={tracks} orderedTrackIds={["a","b","c","d"]} onSelect={noop} onPlay={noop}/>);
  expect(screen.getByText("3 of 4 tracks loaded")).toBeInTheDocument();
  expect(screen.getAllByText("Energy unknown")).toHaveLength(2);
  expect(screen.getByText("Previous track not loaded")).toBeInTheDocument();
  expect(screen.queryByText("No harmonic key match")).not.toBeInTheDocument();
  expect(screen.getAllByRole("meter")).toHaveLength(1);
 });

 it("recognizes harmonic neighbors, relative modes and notation aliases without treating other keys as a verdict",()=>{
  expect(compareFlowKeys("12A","1A")).toBe("match");
  expect(compareFlowKeys("8A","8B")).toBe("match");
  expect(compareFlowKeys("Am","1m")).toBe("match");
  expect(compareFlowKeys("C major","8A")).toBe("match");
  expect(compareFlowKeys("8A","9B")).toBe("outside");
  expect(compareFlowKeys(null,"8A")).toBe("unknown");
  expect(compareFlowKeys("13A","8A")).toBe("unknown");
 });

 it("labels unrecognized keys and out-of-range measurements as unknown, including the first slot",()=>{
  render(<PlaylistFlow tracks={[track("a",{key:"13A",energy:0,bpm:-120}),track("b",{key:"2B",energy:11}),track("c",{key:"8A"})]} orderedTrackIds={["a","b","c"]} onSelect={noop} onPlay={noop}/>);
  const first=screen.getAllByRole("listitem")[0];
  expect(within(first).getByText("Key unknown")).toHaveAttribute("title","13A");
  expect(within(first).getByText("Energy unknown")).toBeInTheDocument();
  expect(within(first).getByText("BPM unknown")).toBeInTheDocument();
  expect(screen.getByText("No harmonic key match")).toBeInTheDocument();
  expect(screen.getAllByRole("meter")).toHaveLength(1);
 });

 it("selects and previews by keyboard through distinct callbacks and reflects selection",async()=>{
  const onSelect=vi.fn(),onPlay=vi.fn(),user=userEvent.setup();
  render(<PlaylistFlow tracks={[track("a")]} orderedTrackIds={["a"]} selectedTrackId="a" onSelect={onSelect} onPlay={onPlay}/>);
  const select=screen.getByRole("button",{name:"Select Track a"});
  expect(select).toHaveAttribute("aria-pressed","true");
  select.focus();await user.keyboard(" ");
  expect(onSelect).toHaveBeenCalledWith("a");
  expect(onPlay).not.toHaveBeenCalled();
  await user.tab();await user.keyboard("{Enter}");
  expect(onPlay).toHaveBeenCalledWith("a");
 });

 it("bounds the DOM for large playlists and labels both loaded scope and displayed range",()=>{
  const tracks=Array.from({length:1000},(_,i)=>track(String(i))),ids=tracks.map(t=>t.id);
  render(<PlaylistFlow tracks={tracks} orderedTrackIds={ids} totalCount={1200} onSelect={noop} onPlay={noop}/>);
  expect(screen.getAllByRole("listitem")).toHaveLength(40);
  expect(screen.getByText("1,000 of 1,200 tracks loaded")).toBeInTheDocument();
  expect(screen.getByText("Showing loaded tracks 1–40 of 1,000")).toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Previous tracks"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Next tracks"}));
  expect(screen.getByText("Showing loaded tracks 41–80 of 1,000")).toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Select Track 40"})).toBeInTheDocument();
  expect(screen.queryByRole("button",{name:"Select Track 0"})).not.toBeInTheDocument();
 });

 it("distinguishes an empty playlist from membership whose metadata has not loaded",()=>{
  const {rerender}=render(<PlaylistFlow tracks={[]} orderedTrackIds={[]} onSelect={noop} onPlay={noop}/>);
  expect(screen.getByText("Add tracks to see this playlist’s flow.")).toBeInTheDocument();
  rerender(<PlaylistFlow tracks={[]} orderedTrackIds={["a"]} onSelect={noop} onPlay={noop}/>);
  expect(screen.getByText("No track details loaded yet.")).toBeInTheDocument();
  expect(screen.getByText("0 of 1 tracks loaded")).toBeInTheDocument();
 });
});
