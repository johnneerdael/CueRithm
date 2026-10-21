import {act,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import MetadataEditor from "../components/MetadataEditor";
import {editorBridge} from "./editor-fixtures";
import type {MetadataEditPreview,MetadataArtworkAsset,MetadataEditBatch} from "../editor-types";
describe("native metadata editor",()=>{
 it("lets the user explicitly abandon a failed local edit before navigating",async()=>{
  const {bridge}=editorBridge(),close=vi.fn();bridge.updateMetadataDraft=vi.fn(async()=>{throw Error("Draft storage unavailable");});
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={close} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.change(await screen.findByRole("combobox",{name:"Title action"}),{target:{value:"set"}});
  await screen.findByText("Draft storage unavailable");
  fireEvent.click(screen.getByRole("button",{name:"Discard unsaved changes"}));
  const confirm=screen.getByRole("alertdialog",{name:"Discard unsaved changes?"});
  expect(within(confirm).getByText(/last saved native draft/)).toBeInTheDocument();
  fireEvent.click(within(confirm).getByRole("button",{name:"Discard unsaved changes"}));
  await screen.findByText("Unsaved edits discarded");
  expect(bridge.discardMetadataDraft).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Back to tracks"}));await waitFor(()=>expect(close).toHaveBeenCalledTimes(1));
 });
 it("displays the actual flat measured payload independently from editable file overrides",async()=>{
  const {bridge}=editorBridge();render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  const measured=await screen.findByRole("region",{name:"Measured analysis"});expect(within(measured).getByText("8A")).toBeInTheDocument();expect(within(measured).getByText("128.00")).toBeInTheDocument();expect(within(measured).getByText("6 / 10")).toBeInTheDocument();expect(within(measured).queryByRole("textbox")).not.toBeInTheDocument();
 });
 it("retains explicit ID3v2.4 conversion permission in the native draft",async()=>{
  const {bridge}=editorBridge(),snapshot=bridge.metadataEditorSnapshot;
  bridge.metadataEditorSnapshot=vi.fn(async(...args:Parameters<typeof snapshot>)=>{const value=await snapshot(...args);value.tracks[0].native.capabilities.id3_version="2.3";return value;});
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  const permission=await screen.findByRole("checkbox",{name:"Allow conversion to ID3v2.4"});expect(permission).not.toBeChecked();fireEvent.click(permission);
  await waitFor(()=>expect(bridge.updateMetadataDraft).toHaveBeenLastCalledWith(expect.any(String),expect.any(Number),expect.objectContaining({fields:expect.objectContaining({allow_id3_upgrade:true})})));
 });
 it("makes mixed Keep/Set/Clear explicit and preserves contributor values without comma splitting",async()=>{
  const {bridge}=editorBridge(["a","b"]);
  render(<MetadataEditor bridge={bridge} trackIds={["a","b"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  const title=await screen.findByRole("textbox",{name:"Title"});expect(title).toBeDisabled();expect(title).toHaveAttribute("placeholder",expect.stringContaining("Mixed"));
  fireEvent.change(screen.getByRole("combobox",{name:"Artists action"}),{target:{value:"set"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Artists"}),{target:{value:"Nora Vale\nGuest, Jr."}});
  fireEvent.change(screen.getByRole("combobox",{name:"Title action"}),{target:{value:"clear"}});
  await waitFor(()=>expect(bridge.updateMetadataDraft).toHaveBeenLastCalledWith(expect.any(String),expect.any(Number),expect.objectContaining({fields:expect.objectContaining({text:expect.objectContaining({artist:{kind:"set",values:["Nora Vale","Guest, Jr."]},title:{kind:"clear"}})})})));
  expect(bridge.playerLoad).not.toHaveBeenCalled();
  expect(screen.getByRole("button",{name:"Save changes"})).toBeDisabled();
 });
 it("restores an unfinished date from its native draft after closing and remounting",async()=>{
  const {bridge}=editorBridge();const close=vi.fn();
  const first=render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={close} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.change(await screen.findByRole("combobox",{name:"Release date action"}),{target:{value:"set"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Release date"}),{target:{value:"2026-"}});
  fireEvent.click(screen.getByRole("button",{name:"Back to tracks"}));
  await waitFor(()=>expect(close).toHaveBeenCalledTimes(1));first.unmount();
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  expect(await screen.findByRole("textbox",{name:"Release date"})).toHaveValue("2026-");
  expect(screen.getByRole("button",{name:"Preview changes"})).toBeDisabled();
  expect(bridge.createMetadataDraft).toHaveBeenCalledTimes(1);
 });
 it("does not restore an in-flight preview after the selected file changes externally",async()=>{
  const {bridge}=editorBridge();let changed:Parameters<typeof bridge.onLibraryChange>[0]=()=>{};
  bridge.onLibraryChange=vi.fn(async listener=>{changed=listener;return()=>{};});
  const preview=bridge.previewMetadataEdits;let finish!:(value:MetadataEditPreview)=>void;
  bridge.previewMetadataEdits=vi.fn(async(operationId:string,id:string,revision:number)=>{const value=await preview(operationId,id,revision);return new Promise<MetadataEditPreview>(resolve=>{finish=()=>resolve(value);});});
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Preview changes"}));
  await waitFor(()=>expect(bridge.previewMetadataEdits).toHaveBeenCalledTimes(1));
  changed({track_ids:[]});finish({} as MetadataEditPreview);
  await waitFor(()=>expect(screen.queryByText("Checking selected files…")).not.toBeInTheDocument());
  expect(screen.getByRole("button",{name:"Save changes"})).toBeDisabled();
 });
 it("recovers actual partial history after a rejected publication without hiding the original error",async()=>{
  const {bridge,history}=editorBridge();
  bridge.applyMetadataEdits=vi.fn(async()=>{history.push({id:"partial",draft_id:"draft-1",draft_revision:1,state:"partial",created:2,can_undo:true,can_recover:false,error:"One track failed",rows:[{track_id:"a",title:"Track a",before:{text:{},ratings:[],comments:[],artwork:[],capabilities:{},release_date:null,original_date:null,track_number:null,track_total:null,disc_number:null,disc_total:null,bpm:null},after:{text:{},ratings:[],comments:[],artwork:[],capabilities:{},release_date:null,original_date:null,track_number:null,track_total:null,disc_number:null,disc_total:null,bpm:null},outcome:"failed",fields:[{field:"title",status:"failed",reason:"File became read only"}],error:"File became read only"}]});throw Error("Connection interrupted after publication");});
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Save changes"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Save changes"}));
  await screen.findByText("Connection interrupted after publication");
  expect(await screen.findByText("One track failed")).toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Undo saved changes"})).toBeInTheDocument();
 });
 it("updates the exact queued publication from native history without remounting",async()=>{
  const {bridge,history}=editorBridge();let changed:()=>void=()=>{};
  bridge.onMetadataEditChange=vi.fn(async listener=>{changed=listener;return()=>{};});
  const apply=bridge.applyMetadataEdits;
  bridge.applyMetadataEdits=vi.fn(async(...args:Parameters<typeof apply>)=>{const batch=await apply(...args);batch.state="queued";batch.can_undo=false;history[0]=batch;return structuredClone(batch);});
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Save changes"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Save changes"}));
  await screen.findByRole("heading",{name:"Publication · queued"});history[0]={...history[0],state:"saved",can_undo:true};changed();
  expect(await screen.findByRole("heading",{name:"Publication · saved"})).toBeInTheDocument();
 });
 it("refreshes a queued receipt when completion was emitted before Apply returned",async()=>{
  const {bridge,history}=editorBridge();let changed:()=>void=()=>{};
  bridge.onMetadataEditChange=vi.fn(async listener=>{changed=listener;return()=>{};});
  const apply=bridge.applyMetadataEdits;
  bridge.applyMetadataEdits=vi.fn(async(...args:Parameters<typeof apply>)=>{const saved=await apply(...args);history[0]={...saved,state:"saved",can_undo:true};changed();return {...saved,state:"queued",can_undo:false};});
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Save changes"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Save changes"}));
  expect(await screen.findByRole("heading",{name:"Publication · saved"})).toBeInTheDocument();
  expect(bridge.metadataEditBatch).toHaveBeenCalledWith(history[0].id);expect(screen.getByRole("button",{name:"Undo saved changes"})).toBeEnabled();
 });
 it("does not let a late queued-batch refresh replace a newer editor draft",async()=>{
  const {bridge}=editorBridge(),apply=bridge.applyMetadataEdits;let finish!:(value:Awaited<ReturnType<typeof bridge.metadataEditBatch>>)=>void;
  let saved!:Awaited<ReturnType<typeof bridge.metadataEditBatch>>;
  bridge.applyMetadataEdits=vi.fn(async(...args:Parameters<typeof apply>)=>{saved=await apply(...args);return {...saved,state:"queued",can_undo:false};});
  bridge.metadataEditBatch=vi.fn(()=>new Promise<MetadataEditBatch>(resolve=>{finish=resolve;}));
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Save changes"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Save changes"}));
  await waitFor(()=>expect(bridge.metadataEditBatch).toHaveBeenCalledTimes(1));fireEvent.click(screen.getByRole("button",{name:"New edit"}));
  fireEvent.change(await screen.findByRole("combobox",{name:"Title action"}),{target:{value:"set"}});fireEvent.change(screen.getByRole("textbox",{name:"Title"}),{target:{value:"Newer draft choice"}});
  await act(async()=>finish(saved));
  expect(screen.getByRole("textbox",{name:"Title"})).toHaveValue("Newer draft choice");expect(screen.queryByRole("heading",{name:"Publication · saved"})).not.toBeInTheDocument();
 });
 it("waits for a chosen artwork asset before persisting the final navigation draft",async()=>{
  const {bridge}=editorBridge();let choose!:(value:{id:string;mime:string;sha256:string;bytes:number})=>void;
  bridge.chooseMetadataArtwork=vi.fn(()=>new Promise<MetadataArtworkAsset>(resolve=>{choose=resolve;}));
  bridge.metadataArtworkAsset=vi.fn(async()=>"data:image/png;base64,iVBORw0KGgo=");const close=vi.fn();
  render(<MetadataEditor bridge={bridge} trackIds={["a"]} onClose={close} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Choose image"}));await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"Back to tracks"}));await Promise.resolve();});
  expect(close).not.toHaveBeenCalled();
  choose({id:"opaque-asset",mime:"image/png",sha256:"art",bytes:10});
  await waitFor(()=>expect(close).toHaveBeenCalledTimes(1));
  expect(bridge.updateMetadataDraft).toHaveBeenLastCalledWith(expect.any(String),expect.any(Number),expect.objectContaining({artwork:[{target:null,asset_id:"opaque-asset",kind:null,description:""}]}));
 });
});
