import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import tauriConfig from "../../src-tauri/tauri.conf.json";
import {fakeBridge,track} from "./bridge";
import {workspace,playlistNode} from "./playlist-fixtures";
import {pointerDrag} from "./pointer-drag";
describe("track drag and drop",()=>{
 it("adds the dragged track selection to an ordinary playlist, and cannot drop into a smart playlist",async()=>{
  const bridge=fakeBridge([track("a"),track("b")]);const state=workspace();state.tree.nodes.push(playlistNode("smart","Smart","smartlist"));bridge.getPlaylists=vi.fn(async()=>state);
  render(<App bridge={bridge}/>);const title=await screen.findByText("Track a");fireEvent.click(title);
  const smart=screen.getByRole("button",{name:"Smart playlist Smart"}).closest("li")!;pointerDrag(title.closest('[role="row"]')!,smart);expect(bridge.editPlaylist).not.toHaveBeenCalled();
  const house=screen.getByRole("button",{name:"Playlist House"}).closest("[draggable]")!;pointerDrag(title.closest('[role="row"]')!,house);
  await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenCalledWith({type:"add_members",id:"house",track_ids:["a"]},7));expect(bridge.enqueuePaths).not.toHaveBeenCalled();
 });
 it("reorders matching manual slots by dropping a selection before another row",async()=>{
  const bridge=fakeBridge([track("a"),track("b")]);bridge.getPlaylists=vi.fn(async()=>workspace());
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"House 2"});const from=screen.getByText("Track b").closest('[role="row"]')!,to=screen.getByText("Track a").closest('[role="row"]')!;
  expect(from).toHaveAttribute("draggable","false");pointerDrag(from,to);
  await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenCalledWith({type:"reorder_members",id:"house",track_ids:["b","a"]},7));expect(bridge.playerLoad).not.toHaveBeenCalled();
 });
 it("cancels a pointer drag with Escape while native file-drop handling stays enabled",async()=>{
  expect(tauriConfig.app.windows.every(window=>(window as {dragDropEnabled?:boolean}).dragDropEnabled!==false)).toBe(true);
  const bridge=fakeBridge([track("a")]);bridge.getPlaylists=vi.fn(async()=>workspace());render(<App bridge={bridge}/>);
  const source=(await screen.findByText("Track a")).closest('[role="row"]')!,destination=(await screen.findByRole("button",{name:"Playlist House"})).closest("[data-playlist-drop-id]")!;
  pointerDrag(source,destination,true);expect(bridge.editPlaylist).not.toHaveBeenCalled();expect(document.querySelector(".internal-drag-ghost")).toBeNull();
 });
});
