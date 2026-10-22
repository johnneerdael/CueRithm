import {act,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {beforeEach,describe,expect,it,vi} from "vitest";
import Sync from "../views/Sync";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import type {DjSyncHistoryEntry,FileJournal,SyncPreview} from "../sync-types";
import type {SyncTargetConfiguration} from "../sync-target-types";
import type {PlaylistWorkspace} from "../playlist-types";
import {syncPreferencesKey as preferencesKey} from "../sync-preferences";

beforeEach(()=>{localStorage.removeItem(preferencesKey);});

function prepared(id:string):SyncPreview{return {id,target:{software:"virtual_dj",path:"/VirtualDJ/database.xml",source_root:null},tracks:1,playlists:0,bytes_to_write:1024,files:[],warnings:[],directories:[],deleted_playlists:0,retained_external_playlists:0};}
function enterDestination(){fireEvent.change(screen.getByRole("textbox",{name:"VirtualDJ library folder"}),{target:{value:"/VirtualDJ"}});}

describe("native DJ sync",()=>{
 it("previews Windows references from an explicit Wine setup and invalidates it after mapping edits",async()=>{
  const bridge=fakeBridge([]);bridge.previewDjSync=vi.fn(async()=>prepared("mapped"));
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);enterDestination();
  fireEvent.change(screen.getByRole("combobox",{name:"Application environment"}),{target:{value:"windows_compatibility"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Wine or Proton prefix"}),{target:{value:"/games/prefix"}});
  fireEvent.click(screen.getByRole("button",{name:"Add drive mapping"}));
  fireEvent.change(screen.getByRole("textbox",{name:"Windows root 1"}),{target:{value:"D:\\Music"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Host folder 1"}),{target:{value:"/mnt/music"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledWith(expect.objectContaining({target:expect.objectContaining({environment:{kind:"windows_compatibility",prefix:"/games/prefix",installation:null,mappings:[{windows_root:"D:\\Music",host_root:"/mnt/music"}]}})})));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeEnabled());
  fireEvent.change(screen.getByRole("textbox",{name:"Windows root 1"}),{target:{value:"E:\\Music"}});
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenCalledWith("mapped"));expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeDisabled();
 });
 it("binds a saved normalized database path and invalidates its revision after edits or deletion",async()=>{
  const bridge=fakeBridge([]),saved:SyncTargetConfiguration={id:"normalized-setup",revision:4,name:"Saved decks",target:{kind:"dj_library",target:{software:"virtual_dj",path:"/VirtualDJ/database.xml",source_root:null}}};
  bridge.saveSyncTargetConfiguration=vi.fn(async()=>saved);
  bridge.previewDjSync=vi.fn(async()=>prepared("normalized-preview"));
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);enterDestination();
  fireEvent.change(screen.getByRole("textbox",{name:"Setup name"}),{target:{value:saved.name}});
  await waitFor(()=>expect(screen.getByRole("button",{name:"Save setup"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Save setup"}));
  await waitFor(()=>expect(bridge.saveSyncTargetConfiguration).toHaveBeenCalledWith({id:null,expected_revision:null,name:saved.name,target:{kind:"dj_library",target:{software:"virtual_dj",path:"/VirtualDJ",source_root:null}}}));
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"VirtualDJ library folder"})).toHaveValue("/VirtualDJ/database.xml"));
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledWith(expect.objectContaining({target:{software:"virtual_dj",path:"/VirtualDJ/database.xml",source_root:null},configuration:{id:saved.id,revision:4}})));
  await screen.findByRole("region",{name:"Sync preview"});
  fireEvent.change(screen.getByRole("textbox",{name:"VirtualDJ library folder"}),{target:{value:"/OtherDJ"}});
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenCalledWith("normalized-preview"));
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledTimes(2));
  expect(vi.mocked(bridge.previewDjSync).mock.lastCall?.[0]).not.toHaveProperty("configuration");
  await screen.findByRole("region",{name:"Sync preview"});
  fireEvent.change(screen.getByRole("combobox",{name:"Saved library setup"}),{target:{value:saved.id}});
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"VirtualDJ library folder"})).toHaveValue("/VirtualDJ/database.xml"));
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledTimes(3));
  expect(vi.mocked(bridge.previewDjSync).mock.lastCall?.[0]).toMatchObject({configuration:{id:saved.id,revision:4}});
  await screen.findByRole("region",{name:"Sync preview"});
  fireEvent.click(screen.getByRole("button",{name:"Delete saved setup"}));
  await waitFor(()=>expect(bridge.deleteSyncTargetConfiguration).toHaveBeenCalledWith(saved.id,4));
  await waitFor(()=>expect(screen.queryByRole("region",{name:"Sync preview"})).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledTimes(4));
  expect(vi.mocked(bridge.previewDjSync).mock.lastCall?.[0]).not.toHaveProperty("configuration");
 });
 it.each(["destination","software"] as const)("ignores a normalized save returned after changing %s",async(change)=>{
  const bridge=fakeBridge([]),saved:SyncTargetConfiguration={id:"late-normalized",revision:1,name:"Saved decks",target:{kind:"dj_library",target:{software:"virtual_dj",path:"/VirtualDJ/database.xml",source_root:null}}};
  let finish!:(value:SyncTargetConfiguration)=>void;
  bridge.saveSyncTargetConfiguration=vi.fn(()=>new Promise<SyncTargetConfiguration>(resolve=>{finish=resolve;}));
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);enterDestination();
  fireEvent.change(screen.getByRole("textbox",{name:"Setup name"}),{target:{value:saved.name}});
  await waitFor(()=>expect(screen.getByRole("button",{name:"Save setup"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Save setup"}));
  await waitFor(()=>expect(bridge.saveSyncTargetConfiguration).toHaveBeenCalled());
  if(change==="software")fireEvent.change(screen.getByRole("combobox",{name:"Sync to"}),{target:{value:"engine_dj"}});
  const label=change==="software"?"Engine Library folder or target drive":"VirtualDJ library folder";
  fireEvent.change(screen.getByRole("textbox",{name:label}),{target:{value:"/CurrentChoice"}});
  await act(async()=>finish(saved));
  expect(screen.getByRole("textbox",{name:label})).toHaveValue("/CurrentChoice");
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalled());
  expect(vi.mocked(bridge.previewDjSync).mock.lastCall?.[0]).toMatchObject({target:{software:change==="software"?"engine_dj":"virtual_dj",path:"/CurrentChoice"}});
  expect(vi.mocked(bridge.previewDjSync).mock.lastCall?.[0]).not.toHaveProperty("configuration");
 });
 it("restores destination, scope and options after remount without restoring a preview",async()=>{
  const bridge=fakeBridge([]);
  bridge.discoverDjTargets=vi.fn(async()=>[{software:"pioneer_usb" as const,path:"/Volumes/Discovered"}]);
  const view=render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  fireEvent.change(screen.getByRole("combobox",{name:"Sync to"}),{target:{value:"pioneer_usb"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Target USB drive"}),{target:{value:"/Volumes/Saved"}});
  fireEvent.change(screen.getByRole("combobox",{name:"What to sync"}),{target:{value:"changed"}});
  fireEvent.change(screen.getByRole("combobox",{name:"Cue destination"}),{target:{value:"both"}});
  fireEvent.change(screen.getByRole("combobox",{name:"CDJ compatibility"}),{target:{value:"detect"}});
  fireEvent.click(screen.getByText("Field mappings"));
  fireEvent.change(screen.getByRole("combobox",{name:"Map comment"}),{target:{value:"genre"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await screen.findByRole("region",{name:"Sync preview"});
  view.unmount();
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  await waitFor(()=>expect(bridge.discoverDjTargets).toHaveBeenCalledTimes(2));
  expect(screen.getByRole("combobox",{name:"Sync to"})).toHaveValue("pioneer_usb");
  expect(screen.getByRole("textbox",{name:"Target USB drive"})).toHaveValue("/Volumes/Saved");
  expect(screen.getByRole("combobox",{name:"What to sync"})).toHaveValue("changed");
  expect(screen.getByRole("combobox",{name:"Cue destination"})).toHaveValue("both");
  expect(screen.getByRole("combobox",{name:"CDJ compatibility"})).toHaveValue("detect");
  fireEvent.click(screen.getByText("Field mappings"));
  expect(screen.getByRole("combobox",{name:"Map comment"})).toHaveValue("genre");
  expect(screen.queryByRole("region",{name:"Sync preview"})).not.toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Sync to Pioneer USB"})).toBeDisabled();
  expect(bridge.applyDjSync).not.toHaveBeenCalled();
  expect(bridge.previewDjSync).toHaveBeenCalledTimes(1);
  const saved=JSON.parse(localStorage.getItem(preferencesKey)!);
  expect(Object.keys(saved).sort()).toEqual(["destination","profiles","software","version"]);
  expect(Object.keys(saved.profiles.pioneer_usb).sort()).toEqual(["environment","options","path","playlistId","scope","sourceRoot"]);
 });
 it("keeps independent destination, playlist selection and field mappings for each target",async()=>{
  const bridge=fakeBridge([]);
  const workspace:PlaylistWorkspace={tree:{revision:1,nodes:[{id:"set",name:"Saved set",kind:"playlist",parent_id:null,position:0,favorite_position:null,definition:null,deleted:false,deleted_by:null}]},statuses:[],latest_undo:null,service_error:null};
  const view=render(<Sync bridge={bridge} workspace={workspace} revision={1} onChanged={()=>{}}/>);
  enterDestination();
  fireEvent.change(screen.getByRole("combobox",{name:"What to sync"}),{target:{value:"playlist"}});
  fireEvent.change(screen.getByRole("combobox",{name:"Sync playlist"}),{target:{value:"set"}});
  fireEvent.click(screen.getByRole("checkbox",{name:"Keep existing beatgrids"}));
  fireEvent.click(screen.getByText("Field mappings"));
  fireEvent.change(screen.getByRole("combobox",{name:"Map comment"}),{target:{value:"artist"}});
  fireEvent.change(screen.getByRole("combobox",{name:"Sync to"}),{target:{value:"engine_dj"}});
  expect(screen.getByRole("textbox",{name:"Engine Library folder or target drive"})).toHaveValue("");
  expect(screen.getByRole("combobox",{name:"What to sync"})).toHaveValue("full");
  expect(screen.getByRole("checkbox",{name:"Keep existing beatgrids"})).not.toBeChecked();
  expect(screen.getByRole("combobox",{name:"Map comment"})).toHaveValue("comment");
  fireEvent.change(screen.getByRole("textbox",{name:"Engine Library folder or target drive"}),{target:{value:"/Engine"}});
  fireEvent.click(screen.getByRole("checkbox",{name:"Copy audio to the target drive"}));
  fireEvent.change(screen.getByRole("combobox",{name:"Map comment"}),{target:{value:"genre"}});
  fireEvent.change(screen.getByRole("combobox",{name:"Sync to"}),{target:{value:"virtual_dj"}});
  expect(screen.getByRole("textbox",{name:"VirtualDJ library folder"})).toHaveValue("/VirtualDJ");
  expect(screen.getByRole("combobox",{name:"What to sync"})).toHaveValue("playlist");
  expect(screen.getByRole("combobox",{name:"Sync playlist"})).toHaveValue("set");
  expect(screen.getByRole("checkbox",{name:"Keep existing beatgrids"})).toBeChecked();
  expect(screen.getByRole("combobox",{name:"Map comment"})).toHaveValue("artist");
  view.unmount();
  const remounted=render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  expect(screen.getByRole("combobox",{name:"What to sync"})).toHaveValue("playlist");
  expect(screen.getByRole("button",{name:"Preview sync"})).toBeDisabled();
  remounted.rerender(<Sync bridge={bridge} workspace={workspace} revision={1} onChanged={()=>{}}/>);
  expect(screen.getByRole("combobox",{name:"Sync playlist"})).toHaveValue("set");
  expect(screen.getByRole("button",{name:"Preview sync"})).toBeEnabled();
  remounted.rerender(<Sync bridge={bridge} workspace={{...workspace,tree:{revision:2,nodes:[]}}} revision={2} onChanged={()=>{}}/>);
  expect(screen.getByRole("combobox",{name:"What to sync"})).toHaveValue("playlist");
  expect(screen.getByRole("button",{name:"Preview sync"})).toBeDisabled();
  fireEvent.change(screen.getByRole("combobox",{name:"Sync to"}),{target:{value:"engine_dj"}});
  expect(screen.getByRole("textbox",{name:"Engine Library folder or target drive"})).toHaveValue("/Engine");
  expect(screen.getByRole("checkbox",{name:"Copy audio to the target drive"})).toBeChecked();
  fireEvent.click(screen.getByText("Field mappings"));
  expect(screen.getByRole("combobox",{name:"Map comment"})).toHaveValue("genre");
 });
 it.each(["{broken",JSON.stringify({version:0,software:"pioneer_usb",profiles:{}}),JSON.stringify({version:1,software:"unknown",profiles:{virtual_dj:{path:{unix:[47]},scope:"all",playlistId:7,options:{nearest_color:"false",skip_beatgrids:1,key_notation:"invalid",field_mappings:{comment:"not-a-field"}}}}})])("uses safe defaults for corrupt or unsupported saved choices (%#)",async cached=>{
  localStorage.setItem(preferencesKey,cached);
  const bridge=fakeBridge([]);
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  expect(screen.getByRole("combobox",{name:"Sync to"})).toHaveValue("virtual_dj");
  expect(screen.getByRole("textbox",{name:"VirtualDJ library folder"})).toHaveValue("");
  expect(screen.getByRole("combobox",{name:"What to sync"})).toHaveValue("full");
  expect(screen.getByRole("checkbox",{name:"Use nearest supported colors"})).toBeChecked();
  expect(screen.getByRole("checkbox",{name:"Keep existing beatgrids"})).not.toBeChecked();
  fireEvent.click(screen.getByText("Field mappings"));
  expect(screen.getByRole("combobox",{name:"Map comment"})).toHaveValue("comment");
  expect(screen.getByRole("button",{name:"Preview sync"})).toBeDisabled();
  expect(bridge.previewDjSync).not.toHaveBeenCalled();expect(bridge.applyDjSync).not.toHaveBeenCalled();
 });
 it("opens from the sidebar, discovers a library and requires a current preview to apply",async()=>{
  const bridge=fakeBridge([track("a")]),snapshot=await bridge.getState();
  let finishState!:(value:typeof snapshot)=>void;
  const initialState=new Promise<typeof snapshot>(resolve=>{finishState=resolve;});
  vi.mocked(bridge.getState).mockImplementation(()=>initialState);
  bridge.discoverDjTargets=vi.fn(async()=>[{software:"virtual_dj" as const,path:"/VirtualDJ/database.xml"}]);
  render(<App bridge={bridge}/>);await screen.findByText("Track a");
  // Rows can arrive before the snapshot that supplies Sync's library revision.
  // Complete startup before asserting the successful preview/apply workflow.
  expect(screen.getByText("Opening library…")).toBeInTheDocument();
  await act(async()=>{finishState(snapshot);});
  await screen.findByText("All up to date");
  fireEvent.click(screen.getByRole("button",{name:"Sync"}));
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"VirtualDJ library folder"})).toHaveValue("/VirtualDJ/database.xml"));
  expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await screen.findByRole("region",{name:"Sync preview"});
  expect(bridge.applyDjSync).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("checkbox",{name:"Keep existing beatgrids"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeDisabled());
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenCalledWith("sync"));
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Sync to VirtualDJ"}));
  await waitFor(()=>expect(bridge.applyDjSync).toHaveBeenCalledWith("sync"));
  await screen.findByText("Everything is already synced.");
 });
 it("discards a preview started before the initial library revision arrives",async()=>{
  const bridge=fakeBridge([track("a")]),snapshot=await bridge.getState();
  let finishState!:(value:typeof snapshot)=>void,finishPreview!:(value:SyncPreview)=>void;
  const initialState=new Promise<typeof snapshot>(resolve=>{finishState=resolve;});
  vi.mocked(bridge.getState).mockImplementation(()=>initialState);
  vi.mocked(bridge.previewDjSync).mockImplementationOnce(()=>new Promise(resolve=>{finishPreview=resolve;}));
  bridge.discoverDjTargets=vi.fn(async()=>[{software:"virtual_dj" as const,path:"/VirtualDJ/database.xml"}]);
  render(<App bridge={bridge}/>);await screen.findByText("Track a");
  fireEvent.click(screen.getByRole("button",{name:"Sync"}));
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"VirtualDJ library folder"})).toHaveValue("/VirtualDJ/database.xml"));
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledTimes(1));
  await act(async()=>{finishState(snapshot);});
  await act(async()=>{finishPreview(prepared("startup-preview"));});
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenCalledWith("startup-preview"));
  expect(screen.queryByRole("region",{name:"Sync preview"})).not.toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeDisabled();
  expect(bridge.applyDjSync).not.toHaveBeenCalled();
  await waitFor(()=>expect(screen.getByRole("button",{name:"Preview sync"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await screen.findByRole("region",{name:"Sync preview"});
 });

 it("passes Pioneer options, modified scope and mappings without reusing another target preview",async()=>{
  const bridge=fakeBridge([]);
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  fireEvent.change(screen.getByRole("combobox",{name:"Sync to"}),{target:{value:"pioneer_usb"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Target USB drive"}),{target:{value:"/Volumes/DJ"}});
  fireEvent.change(screen.getByRole("combobox",{name:"What to sync"}),{target:{value:"changed"}});
  fireEvent.change(screen.getByRole("combobox",{name:"Cue destination"}),{target:{value:"both"}});
  fireEvent.change(screen.getByRole("combobox",{name:"CDJ compatibility"}),{target:{value:"auto_fix"}});
  fireEvent.click(screen.getByRole("checkbox",{name:"Clean up unused files"}));
  fireEvent.click(screen.getByRole("checkbox",{name:"All smartlists to playlists"}));
  fireEvent.change(screen.getByRole("combobox",{name:"Convert keys"}),{target:{value:"camelot"}});
  fireEvent.click(screen.getByText("Field mappings"));
  fireEvent.change(screen.getByRole("combobox",{name:"Map comment"}),{target:{value:"genre"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledWith(expect.objectContaining({target:{software:"pioneer_usb",path:"/Volumes/DJ",source_root:null},scope:{type:"changed"},options:expect.objectContaining({usb_cue_destination:"both",usb_cdj_compatibility:"auto_fix",usb_cleanup_unused:true,materialize_smartlists:true,key_notation:"camelot",field_mappings:{comment:"genre"}})})));
  await screen.findByRole("region",{name:"Sync preview"});
  fireEvent.change(screen.getByRole("combobox",{name:"Sync to"}),{target:{value:"djay_pro"}});
  expect(screen.getByRole("button",{name:"Sync to djay Pro"})).toBeDisabled();
  expect(screen.queryByRole("combobox",{name:"Cue destination"})).not.toBeInTheDocument();
 });
 it("retains failed apply recovery in history and invalidates previews when the library revision changes",async()=>{
  const bridge=fakeBridge([]),changed=vi.fn();
  bridge.djSyncHistory=vi.fn(async()=>[{needs_acknowledgement:false,needs_restore_acknowledgement:false,id:"interrupted",target:{software:"serato4" as const,path:"/Serato/Library/root.sqlite",source_root:null},state:"applying" as const,tracks:3,playlists:1,created:1,error:"Drive disconnected"}]);
  const view=render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={changed}/>);
  await screen.findByText("Drive disconnected");
  fireEvent.click(screen.getByRole("button",{name:"Restore backup"}));
  await waitFor(()=>expect(bridge.restoreDjSync).toHaveBeenCalledWith("interrupted"));
  await screen.findByText("Backup restored.");
  fireEvent.change(screen.getByRole("textbox",{name:"VirtualDJ library folder"}),{target:{value:"/VirtualDJ"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeEnabled());
  view.rerender(<Sync bridge={bridge} workspace={null} revision={2} onChanged={changed}/>);
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeDisabled());
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenCalledWith("sync"));
 });
 it("finishes a published sync from history without creating another preview",async()=>{
  const bridge=fakeBridge([]);
  bridge.djSyncHistory=vi.fn(async()=>[{needs_acknowledgement:true,needs_restore_acknowledgement:false,id:"published",target:{software:"virtual_dj" as const,path:"/VirtualDJ/database.xml",source_root:null},state:"applied" as const,tracks:2,playlists:1,created:1,error:"Local acknowledgement interrupted"}]);
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Finish sync"}));
  await waitFor(()=>expect(bridge.applyDjSync).toHaveBeenCalledWith("published"));
  expect(bridge.previewDjSync).not.toHaveBeenCalled();
  await screen.findByText("Sync recovered. Your backup is available below.");
 });

 it("discards replaced previews and releases the current preview on unmount",async()=>{
  const bridge=fakeBridge([]);let count=0;
  bridge.previewDjSync=vi.fn(async()=>prepared(++count===1?"first":"second"));
  const view=render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  enterDestination();fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await screen.findByRole("region",{name:"Sync preview"});
  await waitFor(()=>expect(screen.getByRole("button",{name:"Preview sync"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledTimes(2));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeEnabled());
  expect(bridge.discardDjSync).toHaveBeenNthCalledWith(1,"first");
  view.unmount();
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenNthCalledWith(2,"second"));
  expect(bridge.applyDjSync).not.toHaveBeenCalled();
 });

 it("waits for option-change cleanup before allocating another preview",async()=>{
  const bridge=fakeBridge([]);let finish!:()=>void;
  bridge.discardDjSync=vi.fn(()=>new Promise<void>(resolve=>{finish=resolve;}));
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  enterDestination();fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeEnabled());
  fireEvent.click(screen.getByRole("checkbox",{name:"Keep existing beatgrids"}));
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenCalledWith("sync"));
  fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  expect(bridge.previewDjSync).toHaveBeenCalledTimes(1);
  await act(async()=>{finish();});
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledTimes(2));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeEnabled());
  bridge.discardDjSync=vi.fn(async()=>{});
 });

 it.each(["revision","unmount"] as const)("discards a late preview after %s instead of losing its files",async reason=>{
  const bridge=fakeBridge([]);let finish!:(value:SyncPreview)=>void;
  bridge.previewDjSync=vi.fn(()=>new Promise<SyncPreview>(resolve=>{finish=resolve;}));
  const view=render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  enterDestination();fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(bridge.previewDjSync).toHaveBeenCalledTimes(1));
  if(reason==="revision")view.rerender(<Sync bridge={bridge} workspace={null} revision={2} onChanged={()=>{}}/>);else view.unmount();
  await act(async()=>{finish(prepared("late"));});
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenCalledWith("late"));
  expect(screen.queryByRole("region",{name:"Sync preview"})).not.toBeInTheDocument();
  expect(bridge.applyDjSync).not.toHaveBeenCalled();
 });

 it("discards the current preview explicitly without applying or restoring",async()=>{
  const bridge=fakeBridge([]);
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  enterDestination();fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await screen.findByRole("region",{name:"Sync preview"});
  await waitFor(()=>expect(screen.getByRole("button",{name:"Discard preview"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Discard preview"}));
  await screen.findByText("Preview discarded.");
  expect(bridge.discardDjSync).toHaveBeenCalledWith("sync");
  expect(screen.queryByRole("region",{name:"Sync preview"})).not.toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeDisabled();
  expect(bridge.applyDjSync).not.toHaveBeenCalled();expect(bridge.restoreDjSync).not.toHaveBeenCalled();
 });

 it("renders raw native preview paths without replacing their apply identity",async()=>{
  const bridge=fakeBridge([]);
  bridge.previewDjSync=vi.fn(async()=>({...prepared("raw-plan"),files:[{target:{unix:[47,114,97,119,45,0xff,46,119,97,118]},before_sha256:"before",after_sha256:null,bytes:1}]}));
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  enterDestination();fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await screen.findByText("/raw-\\xff.wav");
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Sync to VirtualDJ"}));
  await waitFor(()=>expect(bridge.applyDjSync).toHaveBeenCalledWith("raw-plan"));
 });

 it("retains a discard action for incomplete Prepared history while keeping recovery actions",async()=>{
  const bridge=fakeBridge([]);
  let history:DjSyncHistoryEntry[]=[
   {id:"orphan",state:"prepared",target:null,tracks:0,playlists:0,created:null,error:"Preparation metadata missing",needs_acknowledgement:false,needs_restore_acknowledgement:false},
   {id:"published",state:"applied",target:prepared("published").target,tracks:1,playlists:0,created:1,error:null,needs_acknowledgement:true,needs_restore_acknowledgement:false},
  ];
  bridge.djSyncHistory=vi.fn(async()=>history);
  bridge.discardDjSync=vi.fn(async id=>{history=history.filter(entry=>entry.id!==id);});
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  await screen.findByText("Preparation metadata missing");
  const region=within(screen.getByRole("region",{name:"Sync history"}));
  expect(region.getAllByRole("button",{name:"Discard preview"})).toHaveLength(1);
  fireEvent.click(region.getByRole("button",{name:"Discard preview"}));
  await screen.findByText("Preview discarded.");
  expect(bridge.discardDjSync).toHaveBeenCalledWith("orphan");
  expect(screen.queryByText("Preparation metadata missing")).not.toBeInTheDocument();
  expect(region.getByRole("button",{name:"Finish sync"})).toBeInTheDocument();
  expect(region.getByRole("button",{name:"Restore backup"})).toBeInTheDocument();
 });

 it("keeps failed Prepared cleanup visible and retryable",async()=>{
  const bridge=fakeBridge([]);let remains=true;
  bridge.djSyncHistory=vi.fn(async()=>remains?[{id:"retry",state:"prepared" as const,target:null,tracks:0,playlists:0,created:null,error:null,needs_acknowledgement:false,needs_restore_acknowledgement:false}]:[]);
  bridge.discardDjSync=vi.fn().mockRejectedValueOnce(new Error("Preview files are in use")).mockImplementation(async()=>{remains=false;});
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Discard preview"}));
  await screen.findByRole("alert");expect(screen.getByRole("alert")).toHaveTextContent("Preview files are in use");
  await waitFor(()=>expect(screen.getByRole("button",{name:"Discard preview"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Discard preview"}));
  await screen.findByText("Preview discarded.");
  expect(bridge.discardDjSync).toHaveBeenCalledTimes(2);
 });

 it("does not cancel an initiated apply when the view unmounts",async()=>{
  const bridge=fakeBridge([]);let finish!:(value:FileJournal)=>void;
  bridge.applyDjSync=vi.fn(()=>new Promise<FileJournal>(resolve=>{finish=resolve;}));
  const view=render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  enterDestination();fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Sync to VirtualDJ"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Sync to VirtualDJ"}));view.unmount();
  await waitFor(()=>expect(bridge.applyDjSync).toHaveBeenCalledWith("sync"));
  expect(bridge.discardDjSync).not.toHaveBeenCalled();
  await act(async()=>{finish({id:"sync",state:"applied",changes:[],directories:[]});});
  expect(bridge.discardDjSync).not.toHaveBeenCalled();
 });

 it.each(["Finish sync","Restore backup"] as const)("disposes the active preview before %s for another journal",async action=>{
  const bridge=fakeBridge([]);let finish!:()=>void;
  bridge.previewDjSync=vi.fn(async()=>prepared("unused"));
  bridge.discardDjSync=vi.fn(()=>new Promise<void>(resolve=>{finish=resolve;}));
  bridge.djSyncHistory=vi.fn(async()=>[{id:"published",state:"applied" as const,target:prepared("published").target,tracks:1,playlists:0,created:1,error:null,needs_acknowledgement:true,needs_restore_acknowledgement:false}]);
  render(<Sync bridge={bridge} workspace={null} revision={1} onChanged={()=>{}}/>);
  enterDestination();fireEvent.click(screen.getByRole("button",{name:"Preview sync"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:action})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:action}));
  await waitFor(()=>expect(bridge.discardDjSync).toHaveBeenCalledWith("unused"));
  expect(bridge.applyDjSync).not.toHaveBeenCalled();expect(bridge.restoreDjSync).not.toHaveBeenCalled();
  await act(async()=>{finish();});
  await waitFor(()=>expect(action==="Finish sync"?bridge.applyDjSync:bridge.restoreDjSync).toHaveBeenCalledWith("published"));
 });

});
