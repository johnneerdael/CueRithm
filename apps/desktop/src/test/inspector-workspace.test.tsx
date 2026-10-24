import {fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import App from "../App";
import {editorBridge} from "./editor-fixtures";
import {track} from "./bridge";
describe("selection-owned metadata inspector",()=>{
 it("refreshes catalog availability without replacing an offline metadata draft or player",async()=>{
  const {bridge}=editorBridge();let access:()=>void=()=>{};
  bridge.onCatalogAccessChange=vi.fn(async listener=>{access=listener;return()=>{};});
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByText("Track a"));fireEvent.click(screen.getByRole("button",{name:"Edit metadata"}));
  fireEvent.change(await screen.findByRole("combobox",{name:"Title action"}),{target:{value:"set"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Title"}),{target:{value:"Offline choice"}});
  await waitFor(()=>expect(bridge.updateMetadataDraft).toHaveBeenLastCalledWith(expect.any(String),expect.any(Number),expect.objectContaining({fields:expect.objectContaining({text:{title:{kind:"set",values:["Offline choice"]}}})})));
  const reads=vi.mocked(bridge.getState).mock.calls.length;access();await waitFor(()=>expect(bridge.getState).toHaveBeenCalledTimes(reads+1));
  expect(screen.getByRole("textbox",{name:"Title"})).toHaveValue("Offline choice");expect(bridge.metadataEditorSnapshot).toHaveBeenCalledTimes(1);expect(bridge.playerLoad).not.toHaveBeenCalled();
 });
 it("edits a different selection while playback continues and restores the table and initiating action",async()=>{
  const {bridge}=editorBridge(["a","b"]);render(<App bridge={bridge}/>);
  fireEvent.doubleClick(await screen.findByText("Track a"));
  await waitFor(()=>expect(bridge.playerLoad).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByText("Track b"));
  const open=screen.getByRole("button",{name:"Edit metadata"});open.focus();fireEvent.click(open);
  await screen.findByRole("textbox",{name:"Title"});
  expect(bridge.metadataEditorSnapshot).toHaveBeenLastCalledWith(expect.any(String),["b"]);
  fireEvent.change(screen.getByRole("combobox",{name:"Title action"}),{target:{value:"set"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Title"}),{target:{value:"Last edit before navigation"}});
  fireEvent.click(screen.getByRole("button",{name:"Back to tracks"}));
  await waitFor(()=>expect(screen.queryByRole("heading",{name:"Edit metadata"})).not.toBeInTheDocument());
  expect(screen.getByRole("table")).toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Edit metadata"})).toHaveFocus();
  expect(bridge.playerLoad).toHaveBeenCalledTimes(1);
  expect(bridge.playerPause).not.toHaveBeenCalled();
  expect(bridge.updateMetadataDraft).toHaveBeenLastCalledWith(expect.any(String),expect.any(Number),expect.objectContaining({fields:expect.objectContaining({text:{title:{kind:"set",values:["Last edit before navigation"]}}})}));
 });
 it("passes native all-page selection into a batch draft rather than only rendered rows",async()=>{
  const ids=Array.from({length:450},(_,i)=>"a"+i),{bridge}=editorBridge(ids);
  bridge.listTracks=vi.fn(async()=>({rows:ids.slice(0,200).map(track),total:450,revision:1,next_cursor:"next"}));
  render(<App bridge={bridge}/>);await screen.findByText("Track a0");
  fireEvent.keyDown(screen.getByRole("table"),{key:"a",metaKey:true});
  await screen.findByText("450 selected");fireEvent.click(screen.getByRole("button",{name:"Edit metadata"}));
  // Avoid repeated full-table accessibility scans while the batch draft resolves.
  await waitFor(()=>expect(within(screen.getByRole("region",{name:"Edit metadata"})).getByRole("textbox",{name:"Title"})).toBeInTheDocument());
  expect(bridge.createMetadataDraft).toHaveBeenCalledWith(expect.any(String),ids);
  expect(bridge.playerLoad).not.toHaveBeenCalled();
 });
 it("refreshes the loaded stable track after unknown-scope native edits without restarting playback",async()=>{
  const {bridge}=editorBridge();let changed:Parameters<typeof bridge.onLibraryChange>[0]=()=>{};
  bridge.onLibraryChange=vi.fn(async handler=>{changed=handler;return()=>{};});
  render(<App bridge={bridge}/>);fireEvent.doubleClick(await screen.findByText("Track a"));await waitFor(()=>expect(screen.getByRole("button",{name:"Pause"})).toBeEnabled());
  await bridge.playerSeek(87);vi.mocked(bridge.playerSeek).mockClear();
  bridge.getTrack=vi.fn(async()=>({track:{...track("a"),title:"Edited player title",display_path:"/managed/new.flac"},analysis:null,source_sha256:"new-metadata-hash",pcm_sha256:"pcm"}));
  changed({track_ids:[]});await screen.findByText("Edited player title");
  expect(bridge.playerLoad).toHaveBeenCalledTimes(1);expect(bridge.playerPause).not.toHaveBeenCalled();expect(bridge.playerSeek).not.toHaveBeenCalled();expect((await bridge.playerStatus()).position_seconds).toBe(87);
 });
});
