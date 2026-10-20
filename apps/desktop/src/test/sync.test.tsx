import {fireEvent,render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import Sync from "../views/Sync";
import App from "../App";
import {fakeBridge,track} from "./bridge";

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

});
