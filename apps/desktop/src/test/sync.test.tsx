import {act,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import Sync from "../views/Sync";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import type {DjSyncHistoryEntry,FileJournal,SyncPreview} from "../sync-types";

function prepared(id:string):SyncPreview{return {id,target:{software:"virtual_dj",path:"/VirtualDJ/database.xml",source_root:null},tracks:1,playlists:0,bytes_to_write:1024,files:[],warnings:[],directories:[],deleted_playlists:0,retained_external_playlists:0};}
function enterDestination(){fireEvent.change(screen.getByRole("textbox",{name:"VirtualDJ library folder"}),{target:{value:"/VirtualDJ"}});}

describe("native DJ sync",()=>{
 it("opens from the sidebar, discovers a library and requires a current preview to apply",async()=>{
  const bridge=fakeBridge([track("a")]);
  bridge.discoverDjTargets=vi.fn(async()=>[{software:"virtual_dj" as const,path:"/VirtualDJ/database.xml"}]);
  render(<App bridge={bridge}/>);await screen.findByText("Track a");
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
