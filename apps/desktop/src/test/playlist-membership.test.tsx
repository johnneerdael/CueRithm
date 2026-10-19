import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import {workspace,playlistNode} from "./playlist-fixtures";
function setup(){const bridge=fakeBridge([track("a"),track("b")]);const state=workspace();state.tree.nodes.push(playlistNode("smart","High energy","smartlist"));bridge.getPlaylists=vi.fn(async()=>state);return bridge;}
describe("manual playlist membership",()=>{
 it("adds selected tracks only to an ordinary playlist",async()=>{
  const bridge=setup();render(<App bridge={bridge}/>);fireEvent.click(await screen.findByText("Track a"));
  const destination=screen.getByLabelText("Destination playlist");expect(screen.queryByRole("option",{name:"High energy"})).not.toBeInTheDocument();fireEvent.change(destination,{target:{value:"house"}});fireEvent.click(screen.getByRole("button",{name:"Add to playlist"}));
  await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenCalledWith({type:"add_members",id:"house",track_ids:["a"]},7));expect(bridge.reprepareTrack).not.toHaveBeenCalled();expect(bridge.saveSettings).not.toHaveBeenCalled();
 });
 it("removes the complete scoped selection without archiving files or restarting playback",async()=>{
  const bridge=setup();bridge.selectTrackIds=vi.fn(async()=>["a","offscreen"]);render(<App bridge={bridge}/>);fireEvent.doubleClick(await screen.findByText("Track a"));await screen.findByRole("button",{name:"Pause"});fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"House 2"});fireEvent.keyDown(window,{key:"a",metaKey:true});await screen.findByText("2 selected");fireEvent.click(screen.getByRole("button",{name:"Remove from playlist"}));
  await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenCalledWith({type:"remove_members",id:"house",track_ids:["a","offscreen"]},7));await waitFor(()=>expect(screen.queryByText("2 selected")).not.toBeInTheDocument());expect(bridge.archiveTrack).not.toHaveBeenCalled();expect(bridge.playerLoad).toHaveBeenCalledTimes(1);
 });
 it("reorders all filtered matching slots while keeping hidden members out of the edit",async()=>{
  const bridge=setup();bridge.listTracks=vi.fn(async q=>({rows:q.search?[{...track("a"),title:"Filtered a"},{...track("b"),title:"Filtered b"}]:[track("a"),track("b")],total:3,revision:1,next_cursor:null}));bridge.selectTrackIds=vi.fn(async()=>["a","b","c"]);
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"House 3"});fireEvent.change(screen.getByRole("searchbox",{name:"Search library"}),{target:{value:"mix"}});fireEvent.click(await screen.findByText("Filtered b"));fireEvent.click(screen.getByRole("button",{name:"Move selected tracks up"}));
  await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenCalledWith({type:"reorder_members",id:"house",track_ids:["b","a","c"]},7));expect(bridge.selectTrackIds).toHaveBeenCalledWith(expect.objectContaining({playlist_id:"house",search:"mix",sort:"playlist_order",descending:false}));expect(bridge.playlistMembers).not.toHaveBeenCalled();
 });
 it("discards a late reorder read when the user switches playlists",async()=>{
  const bridge=setup();let finish!:(ids:string[])=>void;bridge.selectTrackIds=vi.fn(()=>new Promise<string[]>(resolve=>{finish=resolve;}));
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"House 2"});fireEvent.click(screen.getByText("Track b"));fireEvent.click(screen.getByRole("button",{name:"Move selected tracks up"}));await waitFor(()=>expect(finish).toBeDefined());fireEvent.click(screen.getByRole("button",{name:"Playlist Techno"}));finish(["a","b"]);
  await screen.findByText("Track list changed. Select tracks and try again.");expect(bridge.editPlaylist).not.toHaveBeenCalled();expect(screen.getByRole("heading",{name:"Techno 2"})).toBeInTheDocument();
 });
});
