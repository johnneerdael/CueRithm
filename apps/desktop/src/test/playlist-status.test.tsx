import {render,screen,fireEvent} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import {workspace,playlistNode} from "./playlist-fixtures";
describe("smart playlist status",()=>{
 it("shows the evaluation error and last-good result without offering manual membership edits",async()=>{
  const bridge=fakeBridge([track("a")]);const state=workspace();state.tree.nodes.push(playlistNode("smart","Smart","smartlist"));state.statuses=[{playlist_id:"smart",state:"error",error:"Measured waveform is unavailable",evaluated_at:1,next_refresh:null,last_good_revision:3,has_last_good_results:true}];bridge.getPlaylists=vi.fn(async()=>state);
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Smart playlist Smart"}));await screen.findByText("Measured waveform is unavailable");expect(screen.getByText("Showing the last successfully evaluated results.")).toBeInTheDocument();fireEvent.click(await screen.findByText("Track a"));expect(screen.queryByRole("button",{name:"Remove from playlist"})).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:"Move selected tracks up"})).not.toBeInTheDocument();
 });
 it("shows an empty playlist instead of suggesting that the library has no music",async()=>{
  const bridge=fakeBridge([]);bridge.getPlaylists=vi.fn(async()=>workspace());render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"No tracks in this playlist"});expect(screen.queryByRole("heading",{name:"Your library starts here"})).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:"Add audio files"})).not.toBeInTheDocument();
 });
 it.each(["stale","error"] as const)("shows cached smart results for a %s folder without a synthetic aggregate revision",async status=>{
  const bridge=fakeBridge([track("a")]),state=workspace();
  state.tree.nodes.push(playlistNode("nested","Nested","folder","folder"),playlistNode("smart","Smart","smartlist","nested"));
  state.statuses=[{playlist_id:"folder",state:status,error:status==="error"?"Nested: Smart: Source unavailable":null,evaluated_at:null,next_refresh:null,last_good_revision:null,has_last_good_results:true}];
  bridge.getPlaylists=vi.fn(async()=>state);
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Folder Sets"}));
  await screen.findByText("Showing saved results from available smart playlists.");
  expect(await screen.findByText("Track a")).toBeInTheDocument();
  expect(screen.queryByText("No saved results are available yet.")).not.toBeInTheDocument();
 });
 it("distinguishes a folder's first smart evaluation from its existing manual contents",async()=>{
  const bridge=fakeBridge([track("a")]),state=workspace();
  state.tree.nodes.push(playlistNode("manual","Manual","playlist","folder"),playlistNode("smart","Smart","smartlist","folder"));
  state.statuses=[{playlist_id:"folder",state:"pending",error:null,evaluated_at:null,next_refresh:null,last_good_revision:null,has_last_good_results:false}];
  bridge.getPlaylists=vi.fn(async()=>state);
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Folder Sets"}));
  await screen.findByText("Smart playlists have no saved results yet.");
  expect(await screen.findByText("Track a")).toBeInTheDocument();
  expect(screen.queryByText("No saved results are available yet.")).not.toBeInTheDocument();
 });
 it("recognizes successful empty cached smart results in a folder",async()=>{
  const bridge=fakeBridge([]),state=workspace();
  state.tree.nodes.push(playlistNode("smart","Smart","smartlist","folder"));
  state.statuses=[{playlist_id:"folder",state:"error",error:"Smart: Source unavailable",evaluated_at:null,next_refresh:null,last_good_revision:null,has_last_good_results:true}];
  bridge.getPlaylists=vi.fn(async()=>state);
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Folder Sets"}));
  await screen.findByText("Showing saved results from available smart playlists.");
  expect(await screen.findByRole("heading",{name:"No tracks in this playlist"})).toBeInTheDocument();
 });
 it("does not show an evaluation warning for a ready manual-only folder",async()=>{
  const bridge=fakeBridge([track("a")]),state=workspace();
  state.tree.nodes.push(playlistNode("manual","Manual","playlist","folder"));
  state.statuses=[{playlist_id:"folder",state:"ready",error:null,evaluated_at:null,next_refresh:null,last_good_revision:null,has_last_good_results:false}];
  bridge.getPlaylists=vi.fn(async()=>state);
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Folder Sets"}));
  expect(await screen.findByText("Track a")).toBeInTheDocument();
  expect(document.querySelector(".playlist-evaluation-status")).not.toBeInTheDocument();
 });

});
