import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import PlaylistTools from "../views/PlaylistTools";
import {fakeBridge} from "./bridge";
import {workspace} from "./playlist-fixtures";
import type {PlaylistToolPreview} from "../playlist-types";
const preview=():PlaylistToolPreview=>({id:"tool-preview",stamp:{input_revision:1,playlist_revision:7,preference_revision:4},created:Date.now(),expires:Date.now()+60000,changes:[{id:"union",name:"Union",previous_name:null,kind:"playlist",parent_id:null,position:3,track_count:2,definition:null,is_new:true}]});
describe("playlist tools",()=>{
 it("previews combinations and applies only the native preview token",async()=>{
  const bridge=fakeBridge([]);bridge.previewPlaylistTool=vi.fn(async()=>preview());const onApplied=vi.fn(async()=>{});
  render(<PlaylistTools bridge={bridge} workspace={workspace()} initialId="house" onApplied={onApplied} onClose={()=>{}}/>);
  fireEvent.click(screen.getByLabelText("Select Techno"));fireEvent.change(screen.getByLabelText("Result name"),{target:{value:"Union"}});
  expect(screen.getByRole("button",{name:"Apply preview"})).toBeDisabled();fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await screen.findByText("Union",{selector:"strong"});
  expect(bridge.previewPlaylistTool).toHaveBeenCalledWith({type:"combine",source_ids:["house","techno"],parent_id:null,name:"Union",mode:"union"});
  fireEvent.click(screen.getByRole("button",{name:"Apply preview"}));await waitFor(()=>expect(bridge.applyPlaylistTool).toHaveBeenCalledWith("tool-preview",7));expect(onApplied).toHaveBeenCalledTimes(1);expect(bridge.editPlaylist).not.toHaveBeenCalled();
 });
 it("ignores a late preview after editing options and never enables its apply",async()=>{
  const bridge=fakeBridge([]);let finish!:(p:PlaylistToolPreview)=>void;bridge.previewPlaylistTool=vi.fn(()=>new Promise<PlaylistToolPreview>(resolve=>{finish=resolve;}));
  render(<PlaylistTools bridge={bridge} workspace={workspace()} initialId="house" onApplied={async()=>{}} onClose={()=>{}}/>);
  fireEvent.change(screen.getByLabelText("Result name"),{target:{value:"Before"}});fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(finish).toBeDefined());fireEvent.change(screen.getByLabelText("Result name"),{target:{value:"After"}});finish(preview());
  await waitFor(()=>expect(screen.getByRole("button",{name:"Preview changes"})).toBeEnabled());expect(screen.getByRole("button",{name:"Apply preview"})).toBeDisabled();expect(screen.queryByText("Union",{selector:"strong"})).not.toBeInTheDocument();
 });
 it("exposes numbering settings and clears a stale preview on native rejection",async()=>{
  const bridge=fakeBridge([]);bridge.previewPlaylistTool=vi.fn(async()=>preview());bridge.applyPlaylistTool=vi.fn(async()=>{throw Error("Playlist inputs changed. Preview again.");});
  render(<PlaylistTools bridge={bridge} workspace={workspace()} initialId="house" onApplied={async()=>{}} onClose={()=>{}}/>);
  fireEvent.change(screen.getByLabelText("Playlist tool"),{target:{value:"number"}});fireEvent.change(screen.getByLabelText("Starting number"),{target:{value:"10"}});fireEvent.click(screen.getByLabelText("Leading zeroes"));fireEvent.click(screen.getByLabelText("Replace existing number prefixes"));
  fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await screen.findByText("Union",{selector:"strong"});expect(bridge.previewPlaylistTool).toHaveBeenCalledWith({type:"prefix",ids:["house"],prefix:{type:"number",start:10,leading_zero:true,replace_existing:true}});
  fireEvent.click(screen.getByRole("button",{name:"Apply preview"}));await screen.findByText("Playlist inputs changed. Preview again.");expect(screen.getByRole("button",{name:"Apply preview"})).toBeDisabled();
 });
 it.each([
  ["intersection",{type:"combine",source_ids:["house"],parent_id:null,name:"Result",mode:"intersection"}],
  ["absent_from_all",{type:"combine",source_ids:["house"],parent_id:null,name:"Result",mode:"absent_from_all"}],
 ] as const)("supports the %s combination",async(mode,expected)=>{
  const bridge=fakeBridge([]);render(<PlaylistTools bridge={bridge} workspace={workspace()} initialId="house" onApplied={async()=>{}} onClose={()=>{}}/>);fireEvent.change(screen.getByLabelText("Result name"),{target:{value:"Result"}});fireEvent.change(screen.getByLabelText("Combination mode"),{target:{value:mode}});fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(bridge.previewPlaylistTool).toHaveBeenCalledWith(expected));
 });
 it.each(["name_ascending","name_descending","track_count_descending"] as const)("supports sibling sort %s",async order=>{
  const bridge=fakeBridge([]);render(<PlaylistTools bridge={bridge} workspace={workspace()} initialId="house" onApplied={async()=>{}} onClose={()=>{}}/>);fireEvent.change(screen.getByLabelText("Playlist tool"),{target:{value:"sort"}});fireEvent.change(screen.getByLabelText("Playlist sort order"),{target:{value:order}});fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(bridge.previewPlaylistTool).toHaveBeenCalledWith({type:"sort_siblings",parent_id:null,ids:null,order}));
 });
 it.each(["prefix","uppercase"] as const)("supports %s naming changes",async type=>{
  const bridge=fakeBridge([]);render(<PlaylistTools bridge={bridge} workspace={workspace()} initialId="house" onApplied={async()=>{}} onClose={()=>{}}/>);fireEvent.change(screen.getByLabelText("Playlist tool"),{target:{value:type}});if(type==="prefix")fireEvent.change(screen.getByLabelText("Text prefix"),{target:{value:"Set "}});fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(bridge.previewPlaylistTool).toHaveBeenCalledWith(type==="prefix"?{type:"prefix",ids:["house"],prefix:{type:"text",text:"Set "}}:{type:"uppercase",ids:["house"]}));
 });
 it("selects all matching playlists beyond the visible picker page",async()=>{
  const bridge=fakeBridge([]),state=workspace();state.tree.nodes=Array.from({length:250},(_,i)=>({...state.tree.nodes[0],id:`list-${i}`,name:`Set ${i}`,position:i}));render(<PlaylistTools bridge={bridge} workspace={state} initialId={null} onApplied={async()=>{}} onClose={()=>{}}/>);fireEvent.change(screen.getByLabelText("Playlist tool"),{target:{value:"uppercase"}});fireEvent.click(screen.getByRole("button",{name:"Select all 250 matching playlists"}));fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(bridge.previewPlaylistTool).toHaveBeenCalledWith({type:"uppercase",ids:state.tree.nodes.map(n=>n.id)}));
 });
});
