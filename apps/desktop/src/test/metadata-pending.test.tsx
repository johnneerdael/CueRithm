import {fireEvent,render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import MetadataPending from "../components/MetadataPending";
import {editorBridge,nativeValues} from "./editor-fixtures";
describe("paged metadata history",()=>{
 it("renders native millisecond history timestamps without converting them to seconds",async()=>{
  const {bridge,history}=editorBridge(),created=Date.UTC(2026,9,20,12,30);
  history.push({id:"dated",draft_id:"draft",draft_revision:1,state:"saved",created,can_undo:false,can_recover:false,error:null,rows:[]});
  render(<MetadataPending bridge={bridge} onClose={()=>{}} onOpenDraft={()=>{}} onChanged={()=>{}}/>);
  expect(await screen.findByText(new Date(created).toLocaleString()+" · Revision 1")).toBeInTheDocument();
 });
 it("pages lightweight summaries and reads full native detail only when requested",async()=>{
  const {bridge,history}=editorBridge();
  for(let i=0;i<26;i++)history.push({id:"batch-"+i,draft_id:"draft",draft_revision:1,state:"saved",created:i,can_undo:true,can_recover:false,error:null,rows:[{track_id:"a",title:"Private row "+i,before:nativeValues("Before"),after:nativeValues("After"),outcome:"written",fields:[],error:null}]});
  render(<MetadataPending bridge={bridge} onClose={()=>{}} onOpenDraft={()=>{}} onChanged={()=>{}}/>);
  expect(await screen.findByText("1–25 of 26")).toBeInTheDocument();
  expect(bridge.metadataEditBatch).not.toHaveBeenCalled();
  expect(screen.queryByText("Private row 25")).not.toBeInTheDocument();
  fireEvent.click(screen.getAllByRole("button",{name:"Details"})[0]);
  await waitFor(()=>expect(bridge.metadataEditBatch).toHaveBeenCalledWith("batch-25"));
  expect(await screen.findByText("Private row 25")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Older changes"}));
  expect(await screen.findByText("26–26 of 26")).toBeInTheDocument();
  expect(bridge.metadataEditHistoryPage).toHaveBeenLastCalledWith(25,25);
  expect(screen.queryByText("Private row 25")).not.toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Older changes"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Newer changes"}));
  expect(await screen.findByText("1–25 of 26")).toBeInTheDocument();
 });
 it("refreshes durable partial history after a failed recovery and retains the original failure",async()=>{
  const {bridge,history}=editorBridge(),changed=vi.fn();
  history.push({id:"partial",draft_id:"draft",draft_revision:1,state:"partial",created:1,can_undo:false,can_recover:true,error:null,rows:[]});
  bridge.recoverMetadataEdits=vi.fn(async()=>{history[0].state="recovery_required";throw Error("Native recovery interrupted");});
  render(<MetadataPending bridge={bridge} onClose={()=>{}} onOpenDraft={()=>{}} onChanged={changed}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Retry recovery"}));
  expect(await screen.findByText("Native recovery interrupted")).toBeInTheDocument();
  expect(await screen.findByText("recovery required")).toBeInTheDocument();
  expect(changed).toHaveBeenCalledTimes(1);
 });
});
