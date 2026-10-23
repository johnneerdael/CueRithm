import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import PlaylistGenerator from "../views/PlaylistGenerator";
import {fakeBridge} from "./bridge";
import {workspace,smartCatalog} from "./playlist-fixtures";
import type {GenerationPreview} from "../playlist-types";
const preview=():GenerationPreview=>({id:"generation-preview",stamp:{input_revision:1,playlist_revision:7,preference_revision:4},created:Date.now(),expires:Date.now()+60000,folder_id:"folder",new_nodes:1,items:[{id:"generated",parent_id:"folder",name:"House 120 - 129.99",definition:{match_all:true,rules:[]},track_count:2,is_new:true}]});
describe("smart playlist generation",()=>{
 it("previews combined dimensions and applies the immutable generation token",async()=>{
  const bridge=fakeBridge([]);bridge.getSmartCatalog=vi.fn(async()=>smartCatalog());bridge.previewSmartGeneration=vi.fn(async()=>preview());const applied=vi.fn(async()=>{});
  render(<PlaylistGenerator bridge={bridge} workspace={workspace()} initialFolderId="folder" onApplied={applied} onClose={()=>{}}/>);
  await screen.findByLabelText("Generation dimension 1");fireEvent.click(screen.getByRole("button",{name:"Add dimension"}));fireEvent.change(screen.getByLabelText("Generation dimension 2"),{target:{value:"bpm_ranges"}});fireEvent.click(screen.getByLabelText("Combine dimensions"));
  fireEvent.click(screen.getByRole("button",{name:"Preview playlists"}));await screen.findByText("House 120 - 129.99",{selector:"strong"});
  expect(bridge.previewSmartGeneration).toHaveBeenCalledWith({target:{type:"folder",id:"folder"},dimensions:[{type:"field",field:"genre",include_unobserved:false},{type:"bpm_ranges"}],combine:true});
  fireEvent.click(screen.getByRole("button",{name:"Apply preview"}));await waitFor(()=>expect(bridge.applySmartGeneration).toHaveBeenCalledWith("generation-preview",7));expect(applied).toHaveBeenCalledTimes(1);
 });
 it("labels the full energy range as the whole-track rating, 1 to 10, and the star range as 0 to 5",async()=>{
  const bridge=fakeBridge([]);bridge.getSmartCatalog=vi.fn(async()=>smartCatalog());
  render(<PlaylistGenerator bridge={bridge} workspace={workspace()} initialFolderId={null} onApplied={async()=>{}} onClose={()=>{}}/>);
  fireEvent.change(await screen.findByLabelText("Generation dimension 1"),{target:{value:"field:energy"}});
  expect(screen.getByLabelText("Include unobserved values for dimension 1").closest("label")).toHaveTextContent("Include all 1–10 energy values, even without matches");
  fireEvent.change(screen.getByLabelText("Generation dimension 1"),{target:{value:"field:rating"}});
  expect(screen.getByLabelText("Include unobserved values for dimension 1").closest("label")).toHaveTextContent("Include all 0–5 star rating values, even without matches");
 });
 it("requires explicit full-domain energy and invalidates a preview when options change",async()=>{
  const bridge=fakeBridge([]);bridge.getSmartCatalog=vi.fn(async()=>smartCatalog());bridge.previewSmartGeneration=vi.fn(async()=>preview());
  render(<PlaylistGenerator bridge={bridge} workspace={workspace()} initialFolderId={null} onApplied={async()=>{}} onClose={()=>{}}/>);
  fireEvent.change(await screen.findByLabelText("Generation dimension 1"),{target:{value:"field:energy"}});fireEvent.click(screen.getByLabelText("Include unobserved values for dimension 1"));
  fireEvent.click(screen.getByRole("button",{name:"Preview playlists"}));await screen.findByText("House 120 - 129.99",{selector:"strong"});
  expect(bridge.previewSmartGeneration).toHaveBeenCalledWith({target:{type:"new_folder",parent_id:null,name:"Generated playlists"},dimensions:[{type:"field",field:"energy",include_unobserved:true}],combine:false});
  fireEvent.change(screen.getByLabelText("New folder name"),{target:{value:"Changed"}});expect(screen.getByRole("button",{name:"Apply preview"})).toBeDisabled();
 });
 it("uses category IDs and rejects stale tree previews before apply",async()=>{
  const bridge=fakeBridge([]);bridge.getSmartCatalog=vi.fn(async()=>smartCatalog());bridge.previewSmartGeneration=vi.fn(async()=>preview());const state=workspace();
  const view=render(<PlaylistGenerator bridge={bridge} workspace={state} initialFolderId="folder" onApplied={async()=>{}} onClose={()=>{}}/>);
  fireEvent.change(await screen.findByLabelText("Generation dimension 1"),{target:{value:"category:style"}});fireEvent.click(screen.getByRole("button",{name:"Preview playlists"}));await screen.findByText("House 120 - 129.99",{selector:"strong"});
  expect(bridge.previewSmartGeneration).toHaveBeenCalledWith({target:{type:"folder",id:"folder"},dimensions:[{type:"category",category_id:"style"}],combine:false});
  view.rerender(<PlaylistGenerator bridge={bridge} workspace={{...state,tree:{...state.tree,revision:8}}} initialFolderId="folder" onApplied={async()=>{}} onClose={()=>{}}/>);
  expect(screen.getByRole("button",{name:"Apply preview"})).toBeDisabled();expect(bridge.applySmartGeneration).not.toHaveBeenCalled();
 });
});
