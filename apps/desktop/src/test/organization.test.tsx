import {render,screen,fireEvent,waitFor,act} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import App from "../App";
import Organize from "../views/Organize";
import {fakeBridge,track} from "./bridge";
import {defaultOrganization} from "../organization-types";
import type {OrganizationPreview,OrganizationBatch,OrganizationRow} from "../organization-types";
const preview:OrganizationPreview={id:"frozen-plan",revision:"digest",created:1,totals:{total:53,ready:49,unchanged:1,skipped:1,blocked:2},can_apply:true,estimated_bytes:2*1024**3,available_bytes:8*1024**3};
const row:OrganizationRow={id:"row",track_id:"a",title:"Example track",before_path:"/Invented/old.flac",after_path:"/Invented/new.flac",outcome:"ready",reason:null,references:[]};
const batch:OrganizationBatch={id:"batch",created:1,kind:"apply",state:"partial",totals:{total:53,completed:48,unchanged:1,skipped:1,blocked:2,failed:1,recovery_required:0,cancelled:0},can_undo:true,can_recover:false,error:null};
async function setup(){const bridge=fakeBridge([]),state=await bridge.getState();state.settings.settings.organization={...defaultOrganization(),mode:"rename_in_place"};vi.mocked(bridge.previewOrganization).mockResolvedValue(preview);vi.mocked(bridge.organizationPreviewPage).mockImplementation(async(_id,offset,limit)=>({rows:[{...row,id:"row-"+offset}],total:53,offset,limit}));return {bridge,state};}
const scan=async()=>{await waitFor(()=>expect(screen.getByRole("button",{name:"Preview complete plan"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Preview complete plan"}));await screen.findByRole("heading",{name:"Complete plan · 53 tracks"});await waitFor(()=>expect(screen.getByRole("button",{name:"Apply 49 eligible"})).toBeEnabled());};
it("shows complete counts and paged before/after paths, then applies the frozen native plan",async()=>{
 const {bridge,state}=await setup();const changed=vi.fn();vi.mocked(bridge.applyOrganization).mockResolvedValue(batch);vi.mocked(bridge.organizationHistory).mockResolvedValue([batch]);render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={["a","b"]} playlistId={null} workspace={null} onChanged={changed}/>);await scan();
 expect(screen.getByText("49 eligible · 1 unchanged · 1 skipped ·",{exact:false})).toBeInTheDocument();expect(screen.getByText("2 blocked")).toBeInTheDocument();expect(screen.getByText(row.before_path)).toBeInTheDocument();
 expect(screen.getByLabelText("Organization storage estimate")).toHaveTextContent("2 GiB · 8 GiB minimum available");
 fireEvent.click(screen.getByRole("button",{name:"Next"}));await waitFor(()=>expect(bridge.organizationPreviewPage).toHaveBeenLastCalledWith("frozen-plan",50,50));
 fireEvent.click(screen.getByRole("button",{name:"Apply 49 eligible"}));await waitFor(()=>expect(bridge.applyOrganization).toHaveBeenCalledWith(expect.any(String),"frozen-plan","digest"));await screen.findByRole("heading",{name:"Organization · partial"});expect(changed).toHaveBeenCalled();expect(screen.getByText(/48 completed.*1 failed/)).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));await waitFor(()=>expect(bridge.undoOrganization).toHaveBeenCalledWith(expect.any(String),"batch"));
});
it("disposes a late preview after unmount and cancels its planning operation",async()=>{
 const {bridge,state}=await setup();let finish!:(value:OrganizationPreview)=>void;vi.mocked(bridge.previewOrganization).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));const view=render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={()=>{}}/>);await waitFor(()=>expect(screen.getByRole("button",{name:"Preview complete plan"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Preview complete plan"}));await waitFor(()=>expect(bridge.previewOrganization).toHaveBeenCalled());view.unmount();finish(preview);await waitFor(()=>expect(bridge.discardOrganizationPreview).toHaveBeenCalledWith("frozen-plan"));expect(bridge.cancelOrganization).toHaveBeenCalled();expect(bridge.applyOrganization).not.toHaveBeenCalled();
});
it("retains guarded recovery and alternate resolutions",async()=>{
 const {bridge,state}=await setup();vi.mocked(bridge.organizationHistory).mockResolvedValue([{...batch,state:"recovery_required",can_undo:false,can_recover:true}]);vi.mocked(bridge.organizationAlternates).mockResolvedValue([{operation_id:"alternate-op",track_id:"a",display_path:"/Invented/original.flac",state:"conflict",captured_sha256:"old",observed_sha256:"new"},{operation_id:"owned",track_id:"b",display_path:"/Invented/unchanged-owned.flac",state:"active",captured_sha256:"same"}]);render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={()=>{}}/>);
 await screen.findByRole("button",{name:"Retry recovery"});expect(screen.queryByText("/Invented/unchanged-owned.flac")).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:"Undo"})).not.toBeInTheDocument();fireEvent.click(screen.getByRole("button",{name:"Keep managed copy"}));await waitFor(()=>expect(bridge.resolveOrganizationAlternate).toHaveBeenCalledWith("alternate-op","ignore_this_revision",expect.any(String)));fireEvent.click(screen.getByRole("button",{name:"Retry recovery"}));await waitFor(()=>expect(bridge.recoverOrganization).toHaveBeenCalledWith(expect.any(String),"batch"));
});
it("uses native all-page selection rather than the rendered first page",async()=>{
 const rows=Array.from({length:205},(_,i)=>track(String(i))),bridge=fakeBridge(rows);vi.mocked(bridge.listTracks).mockResolvedValue({rows:rows.slice(0,200),total:205,revision:1,next_cursor:"next"});const state=await bridge.getState();state.settings.settings.organization={...defaultOrganization(),mode:"rename_in_place"};vi.mocked(bridge.getState).mockResolvedValue(state);render(<App bridge={bridge}/>);await screen.findByText("Track 0");fireEvent.keyDown(window,{key:"a",ctrlKey:true});await screen.findByText("205 selected");fireEvent.click(screen.getAllByRole("button",{name:"Organize files"}).at(-1)!);await waitFor(()=>expect(screen.getByRole("button",{name:"Preview complete plan"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Preview complete plan"}));await waitFor(()=>expect(bridge.previewOrganization).toHaveBeenCalled());expect(vi.mocked(bridge.previewOrganization).mock.calls[0][1].scope).toEqual({kind:"tracks",track_ids:rows.map(r=>r.id)});fireEvent.click(screen.getByRole("button",{name:"Library"}));await screen.findByText("205 selected");fireEvent.change(screen.getByRole("searchbox",{name:"Search library"}),{target:{value:"new scope"}});await waitFor(()=>expect(screen.queryByText("205 selected")).not.toBeInTheDocument());
});

it("discards a late cancelled plan and never enables Apply",async()=>{
 const {bridge,state}=await setup();let finish!:(value:OrganizationPreview)=>void;vi.mocked(bridge.previewOrganization).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={()=>{}}/>);await waitFor(()=>expect(screen.getByRole("button",{name:"Preview complete plan"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Preview complete plan"}));fireEvent.click(await screen.findByRole("button",{name:"Cancel operation"}));finish(preview);await waitFor(()=>expect(bridge.discardOrganizationPreview).toHaveBeenCalledWith("frozen-plan"));expect(screen.queryByRole("button",{name:"Apply 49 eligible"})).not.toBeInTheDocument();
});
it("scopes a folder through its native ID and invalidates the plan when the library changes",async()=>{
 const {bridge,state}=await setup();vi.mocked(bridge.chooseOrganizationRoot).mockResolvedValue({id:"scope-root",display_path:"/Invented/Folder"});const props={bridge,snapshot:state.settings,selectedIds:[],playlistId:null,workspace:null,onChanged:()=>{}};const view=render(<Organize {...props} revision={1}/>);fireEvent.change(screen.getByLabelText("Organization scope"),{target:{value:"folder"}});fireEvent.click(screen.getByRole("button",{name:"Choose scope folder"}));await screen.findByText("/Invented/Folder");await scan();expect(vi.mocked(bridge.previewOrganization).mock.calls[0][1].scope).toEqual({kind:"folder",root_id:"scope-root",recursive:true});view.rerender(<Organize {...props} revision={2}/>);await waitFor(()=>expect(bridge.discardOrganizationPreview).toHaveBeenCalledWith("frozen-plan"));expect(screen.queryByRole("button",{name:"Apply 49 eligible"})).not.toBeInTheDocument();expect(screen.getByRole("alert")).toHaveTextContent("Library changed");
});
it("cancels a running history batch through its durable ID",async()=>{
 const {bridge,state}=await setup();vi.mocked(bridge.organizationHistory).mockResolvedValue([{...batch,state:"applying",can_undo:false}]);render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={()=>{}}/>);fireEvent.click(await screen.findByRole("button",{name:"Cancel operation"}));await waitFor(()=>expect(bridge.cancelOrganizationBatch).toHaveBeenCalledWith("batch"));expect(bridge.cancelOrganization).not.toHaveBeenCalled();
});
it("refreshes durable recovery history after apply rejects and invalidates the consumed preview",async()=>{
 const {bridge,state}=await setup();const changed=vi.fn();
 vi.mocked(bridge.applyOrganization).mockImplementation(async()=>{vi.mocked(bridge.organizationHistory).mockResolvedValue([{...batch,state:"recovery_required",can_undo:false,can_recover:true,error:"Published destination needs reference recovery"}]);throw Error("Reference repair interrupted");});
 render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={changed}/>);await scan();
 fireEvent.click(screen.getByRole("button",{name:"Apply 49 eligible"}));
 await screen.findByRole("button",{name:"Retry recovery"});
 expect(screen.getByRole("alert")).toHaveTextContent("Reference repair interrupted");
 expect(screen.queryByRole("button",{name:"Apply 49 eligible"})).not.toBeInTheDocument();
 expect(changed).toHaveBeenCalledTimes(1);
 expect(bridge.organizationHistory).toHaveBeenCalledTimes(2);
});
it("retains the publication error when refreshing its durable history also fails",async()=>{
 const {bridge,state}=await setup();
 vi.mocked(bridge.applyOrganization).mockImplementation(async()=>{vi.mocked(bridge.organizationHistory).mockRejectedValue(Error("History unavailable"));throw Error("Publication interrupted");});
 render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={()=>{}}/>);await scan();
 fireEvent.click(screen.getByRole("button",{name:"Apply 49 eligible"}));
 await waitFor(()=>expect(screen.getByRole("alert")).toHaveTextContent("Publication interrupted"));
 expect(bridge.organizationHistory).toHaveBeenCalledTimes(2);
 expect(screen.queryByRole("button",{name:"Apply 49 eligible"})).not.toBeInTheDocument();
});
it("cancels restored pending files through the durable batch without undoing completed changes",async()=>{
 const {bridge,state}=await setup();const changed=vi.fn();
 vi.mocked(bridge.organizationHistory).mockResolvedValue([{...batch,state:"recovery_required",can_undo:false,can_recover:true}]);
 vi.mocked(bridge.cancelOrganizationBatch).mockImplementation(async()=>{vi.mocked(bridge.organizationHistory).mockResolvedValue([{...batch,state:"cancelled",can_undo:false,can_recover:false}]);});
 render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={changed}/>);
 fireEvent.click(await screen.findByRole("button",{name:"Cancel pending files"}));
 await screen.findByText("Organization · cancelled");
 expect(bridge.cancelOrganizationBatch).toHaveBeenCalledWith("batch");
 expect(bridge.undoOrganization).not.toHaveBeenCalled();
 expect(bridge.cancelOrganization).not.toHaveBeenCalled();
 expect(changed).toHaveBeenCalledTimes(1);
});
it("resumes an Incoming checkpoint using its captured settings instead of manual recovery",async()=>{
 const {bridge,state}=await setup();
 vi.mocked(bridge.organizationHistory).mockResolvedValue([{...batch,state:"recovery_required",can_undo:false,can_recover:true,resume_job_id:"incoming-job"}]);
 render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={()=>{}}/>);
 fireEvent.click(await screen.findByRole("button",{name:"Retry in Incoming"}));
 await waitFor(()=>expect(bridge.retryJob).toHaveBeenCalledWith("incoming-job",true));
 expect(bridge.recoverOrganization).not.toHaveBeenCalled();
 expect(screen.queryByRole("button",{name:"Undo"})).not.toBeInTheDocument();
});

it("refreshes resumed Incoming history when the library revision changes",async()=>{
 const {bridge,state}=await setup();
 const pending:OrganizationBatch={...batch,state:"recovery_required",can_undo:false,can_recover:false};
 vi.mocked(bridge.organizationHistory).mockResolvedValue([pending]);
 const props={bridge,snapshot:state.settings,selectedIds:[],playlistId:null,workspace:null,onChanged:()=>{}};
 const view=render(<Organize {...props} revision={1}/>);
 await screen.findByText("Organization · recovery required");
 vi.mocked(bridge.organizationHistory).mockResolvedValue([{...batch,state:"complete",can_undo:true,can_recover:false}]);
 view.rerender(<Organize {...props} revision={2}/>);
 await screen.findByText("Organization · complete");
 expect(bridge.organizationHistory).toHaveBeenCalledTimes(2);
});
it("refreshes batch details after cancelling rows even when the batch state stays unchanged",async()=>{
 const {bridge,state}=await setup();
 const pending:OrganizationBatch={...batch,state:"recovery_required",can_undo:false,can_recover:true};
 vi.mocked(bridge.organizationHistory).mockResolvedValue([pending]);
 vi.mocked(bridge.organizationBatchPage).mockResolvedValue({rows:[{...row,outcome:"recovery_required",reason:"Pending row"}],total:1,offset:0,limit:50});
 vi.mocked(bridge.cancelOrganizationBatch).mockImplementation(async()=>{
  vi.mocked(bridge.organizationHistory).mockResolvedValue([{...pending,totals:{...pending.totals,cancelled:1}}]);
  vi.mocked(bridge.organizationBatchPage).mockResolvedValue({rows:[{...row,outcome:"cancelled",reason:"Pending row cancelled"}],total:1,offset:0,limit:50});
 });
 render(<Organize bridge={bridge} snapshot={state.settings} selectedIds={[]} playlistId={null} workspace={null} onChanged={()=>{}}/>);
 fireEvent.click(await screen.findByRole("button",{name:"Details"}));
 await screen.findByText("Pending row");
 fireEvent.click(screen.getByRole("button",{name:"Cancel pending files"}));
 await screen.findByText("Pending row cancelled");
 expect(screen.queryByText("Pending row")).not.toBeInTheDocument();
});

it("coalesces history refreshes while the same details page is still loading",async()=>{
 const {bridge,state}=await setup();
 let historyReads=0;
 vi.mocked(bridge.organizationHistory).mockImplementation(async()=>[{...batch,totals:{...batch.totals,completed:++historyReads}}]);
 const finish:Array<(value:{rows:typeof row[];total:number;offset:number;limit:number})=>void>=[];
 vi.mocked(bridge.organizationBatchPage).mockImplementation(()=>new Promise(resolve=>{finish.push(resolve);}));
 const props={bridge,snapshot:state.settings,selectedIds:[],playlistId:null,workspace:null,onChanged:()=>{}};
 const view=render(<Organize {...props} revision={1}/>);
 fireEvent.click(await screen.findByRole("button",{name:"Details"}));
 await waitFor(()=>expect(bridge.organizationBatchPage).toHaveBeenCalledTimes(1));
 view.rerender(<Organize {...props} revision={2}/>);
 await waitFor(()=>expect(screen.getByRole("region",{name:"Organization result"})).toHaveTextContent("2 completed"));
 expect(bridge.organizationBatchPage).toHaveBeenCalledTimes(1);
 await act(async()=>{finish[0]({rows:[{...row,reason:"First resolved details"}],total:1,offset:0,limit:50});});
 await screen.findByText("First resolved details");
 await waitFor(()=>expect(bridge.organizationBatchPage).toHaveBeenCalledTimes(2));
 await act(async()=>{finish[1]({rows:[{...row,reason:"Latest resolved details"}],total:1,offset:0,limit:50});});
 await screen.findByText("Latest resolved details");
});
