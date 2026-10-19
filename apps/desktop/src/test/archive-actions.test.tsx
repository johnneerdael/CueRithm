import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import {workspace,playlistNode} from "./playlist-fixtures";

function setup(archived:boolean){
 const bridge=fakeBridge([{...track("a"),archived},{...track("unloaded"),archived:true}]);
 const state=workspace();state.tree.nodes.push(playlistNode("archived","Archived tracks","smartlist"));
 bridge.getPlaylists=vi.fn(async()=>state);
 // A smart list, or folder containing it, can show archived rows in Tracks.
 bridge.listTracks=vi.fn(async q=>({rows:q.playlist_id?bridge.rows.slice(0,1):bridge.rows.filter(r=>!r.archived),total:q.playlist_id?2:0,revision:1,next_cursor:null}));
 return bridge;
}
describe("archive actions in playlist scopes",()=>{
 it("restores an archived row from a smart list instead of archiving it again",async()=>{
  const bridge=setup(true);render(<App bridge={bridge}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Smart playlist Archived tracks"}));
  fireEvent.click(await screen.findByRole("button",{name:"Actions for Track a"}));
  fireEvent.click(screen.getByRole("menuitem",{name:"Restore to library"}));
  await waitFor(()=>expect(bridge.archiveTrack).toHaveBeenCalledWith("a",false));
 });
 it("archives an active row in the same playlist scope",async()=>{
  const bridge=setup(false);render(<App bridge={bridge}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Smart playlist Archived tracks"}));
  fireEvent.click(await screen.findByRole("button",{name:"Actions for Track a"}));
  fireEvent.click(screen.getByRole("menuitem",{name:"Archive track"}));
  await waitFor(()=>expect(bridge.archiveTrack).toHaveBeenCalledWith("a",true));
 });
 it.each([true,false])("offers explicit archive and restore for the complete unloaded selection (loaded archived=%s)",async archived=>{
  const bridge=setup(archived);bridge.selectTrackIds=vi.fn(async()=>["a","unloaded"]);
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Folder Sets"}));
  await screen.findByText("Track a");fireEvent.keyDown(window,{key:"a",metaKey:true});await screen.findByText("2 selected");
  expect(screen.getByRole("button",{name:"Archive"})).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Restore"}));
  await waitFor(()=>expect(vi.mocked(bridge.archiveTrack).mock.calls).toEqual([["a",false],["unloaded",false]]));
  fireEvent.keyDown(window,{key:"a",ctrlKey:true});await screen.findByText("2 selected");
  fireEvent.click(screen.getByRole("button",{name:"Archive"}));
  await waitFor(()=>expect(vi.mocked(bridge.archiveTrack).mock.calls).toEqual([["a",false],["unloaded",false],["a",true],["unloaded",true]]));
 });
});
