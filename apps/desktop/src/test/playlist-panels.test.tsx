import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import {workspace,smartCatalog,playlistNode} from "./playlist-fixtures";
import type {PlaylistToolPreview} from "../playlist-types";
describe("playlist tool workspace",()=>{
 it("opens tools and generation from the library while preserving playback",async()=>{
  const bridge=fakeBridge([track("a")]);bridge.getPlaylists=vi.fn(async()=>workspace());bridge.getSmartCatalog=vi.fn(async()=>smartCatalog());
  render(<App bridge={bridge}/>);fireEvent.doubleClick(await screen.findByText("Track a"));await screen.findByRole("button",{name:"Pause"});
  fireEvent.click(screen.getByRole("button",{name:"Playlist tools"}));await screen.findByRole("heading",{name:"Playlist tools"});
  fireEvent.click(screen.getByRole("button",{name:"Generate playlists"}));await screen.findByRole("heading",{name:"Generate playlists"});await screen.findByLabelText("Generation dimension 1");
  fireEvent.click(screen.getByRole("button",{name:"Back to library"}));await screen.findByRole("heading",{name:"Tracks 1"});
  expect(bridge.playerLoad).toHaveBeenCalledTimes(1);expect(bridge.playerPause).not.toHaveBeenCalled();expect(bridge.saveSettings).not.toHaveBeenCalled();
 });
 it("publishes applied tool changes into the sidebar and durable undo",async()=>{
  const bridge=fakeBridge([track("a")]);let state=workspace();bridge.getPlaylists=vi.fn(async()=>state);
  const preview:PlaylistToolPreview={id:"saved",stamp:{input_revision:1,playlist_revision:7,preference_revision:4},created:Date.now(),expires:null,changes:[{id:"copy",name:"Snapshot",previous_name:null,kind:"playlist",parent_id:null,position:3,track_count:1,definition:null,is_new:true}]};
  bridge.previewPlaylistTool=vi.fn(async()=>preview);bridge.applyPlaylistTool=vi.fn(async()=>{state={...state,tree:{revision:8,nodes:[...state.tree.nodes,playlistNode("copy","Snapshot")]},latest_undo:"saved-edit"};return {edit_id:"saved-edit",revision:8,created_ids:["copy"],affected_ids:["copy"]};});
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"House 1"});fireEvent.click(screen.getByRole("button",{name:"Playlist tools"}));
  fireEvent.change(screen.getByLabelText("Playlist tool"),{target:{value:"snapshot"}});fireEvent.change(screen.getByLabelText("Result name"),{target:{value:"Snapshot"}});fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await screen.findByText("Snapshot",{selector:"strong"});fireEvent.click(screen.getByRole("button",{name:"Apply preview"}));
  await screen.findByRole("button",{name:"Playlist Snapshot"});await waitFor(()=>expect(screen.getByRole("button",{name:"Undo last playlist change"})).toBeEnabled());expect(screen.getByRole("heading",{name:"Playlist tools"})).toBeInTheDocument();expect(bridge.applyPlaylistTool).toHaveBeenCalledWith("saved",7);
 });
});
