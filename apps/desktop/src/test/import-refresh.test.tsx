import {describe,it,expect,vi} from "vitest";
import {render,screen,waitFor,fireEvent} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {LibraryImport} from "../views/LibraryImport";
import {ImportHistory} from "../views/ImportHistory";
import {importFixtureApi,importInspection,importPreview} from "./import-fixtures";
import {defaultImportPolicy,type ImportOperation} from "../import-types";
const saved=()=>({...defaultImportPolicy(),analysis_fields:{energy:"replace_selected" as const}});
const operation=():ImportOperation=>({operation_id:"old-operation",source_id:"source-one",state:"processing_blocked",created:1,updated:2,policy:saved(),result:{operation_id:"old-operation",tracks_created:1,fields_changed:3,collections_changed:0,processing_requested:true},can_recover:false,can_undo:false,can_retry:true,can_review:true,stages:{proposed:1,blocked:1},issues:[{item_id:"item-one",stage:"metadata.enrichment",state:"blocked",message:"Provider credentials are unavailable"}]});
function fixture(){return importFixtureApi({importOperationPage:vi.fn(async()=>({records:[operation()],total:1,next_cursor:null})),inspectImportRefresh:vi.fn(async()=>structuredClone(importInspection))});}
describe("import refresh and recovery",()=>{
 it("refresh_shows_saved_policy_and_keeps_local_conflict_default",async()=>{
  const api=fixture();api.importPreviewPage=vi.fn(async()=>({effects:[{external_id:"native-101",item_id:"item-one",kind:"track",fields:[{field:"metadata.title",source:{presence:"value",value:"Source title"},local:{presence:"value",value:"Local edit"},proposed:null,conflict:true,blocked_reason:null}]}],total:1,next_cursor:null}));
  render(<LibraryImport api={api} initialSourceId="source-one" onClose={()=>{}}/>);
  expect(await screen.findByText("Imported title")).toBeVisible();
  expect(api.inspectImportRefresh).toHaveBeenCalledWith(expect.any(String),"source-one");
  await userEvent.click(screen.getByRole("button",{name:"Processing choices"}));
  expect(screen.getByLabelText("Analyse energy")).toBeChecked();
  await userEvent.click(screen.getByRole("button",{name:"Review import"}));
  expect(await screen.findByLabelText("Conflict decision")).toHaveValue("keep_local");
  expect(vi.mocked(api.previewImport).mock.lastCall![1].policy).toEqual(saved());
  expect(vi.mocked(api.previewImport).mock.lastCall![1].decisions).toEqual([]);
 });
 it("unchanged_refresh_has_no_effects",async()=>{
  const api=fixture();api.previewImport=vi.fn(async()=>({...importPreview,tracks_created:0,fields_changed:0,collections_changed:0,baselines_changed:0}));
  render(<LibraryImport api={api} initialSourceId="source-one" onClose={()=>{}}/>);
  await userEvent.click(await screen.findByRole("button",{name:"Processing choices"}));
  await userEvent.click(screen.getByRole("button",{name:"Review import"}));
  expect(await screen.findByText(/0 new tracks · 0 field changes · 0 collection changes/)).toBeVisible();
 });
 it("partial_scan_is_not_source_deletion",async()=>{
  const api=fixture();api.inspectImportRefresh=vi.fn(async()=>({...structuredClone(importInspection),header:{...importInspection.header,completeness:"partial_with_errors" as const}}));
  render(<LibraryImport api={api} initialSourceId="source-one" onClose={()=>{}}/>);
  expect(await screen.findByText(/Missing records are not source deletions/)).toBeVisible();
  expect(screen.queryByRole("button",{name:/delete music/i})).not.toBeInTheDocument();
 });
 it("custom_conflict_requires_new_preview",async()=>{
  const api=fixture();api.importPreviewPage=vi.fn(async()=>({effects:[{external_id:"native-101",item_id:"item-one",kind:"track",fields:[{field:"metadata.title",source:{presence:"value",value:"Source title"},local:{presence:"value",value:"Local edit"},proposed:null,conflict:true,blocked_reason:null}]}],total:1,next_cursor:null}));
  render(<LibraryImport api={api} initialSourceId="source-one" onClose={()=>{}}/>);
  await userEvent.click(await screen.findByRole("button",{name:"Processing choices"}));await userEvent.click(screen.getByRole("button",{name:"Review import"}));
  await userEvent.selectOptions(await screen.findByLabelText("Conflict decision"),"custom");
  fireEvent.change(screen.getByLabelText("Custom value JSON"),{target:{value:JSON.stringify({presence:"value",value:"Reviewed title"})}});
  await userEvent.click(screen.getByRole("button",{name:"Use custom value — rebuild preview"}));
  expect(api.applyImport).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button",{name:"Review import"}));
  await waitFor(()=>expect(api.previewImport).toHaveBeenCalledTimes(2));
  expect(vi.mocked(api.previewImport).mock.lastCall![1].decisions[0].decision).toEqual({choice:"custom",value:{presence:"value",value:"Reviewed title"}});
 });
 it("recovery_and_undo_respect_later_edits",async()=>{
  const api=fixture();const row=operation();row.can_recover=true;row.can_undo=true;row.can_retry=false;row.can_review=false;
  api.importOperationPage=vi.fn(async()=>({records:[row],total:1,next_cursor:null}));
  api.recoverImportOperation=vi.fn(async()=>({recovered:0,recovery_required:1,errors:["Source changed; review required"]}));
  api.undoImportOperation=vi.fn(async()=>({undone:0,blocked:1,errors:["Later title edit prevents Undo"]}));
  render(<ImportHistory api={api} onRefresh={()=>{}} onReview={()=>{}}/>);
  await userEvent.click(await screen.findByRole("button",{name:"Recover effects"}));
  expect(await screen.findByText("Source changed; review required")).toBeVisible();
  await userEvent.click(screen.getByRole("button",{name:"Undo effects"}));
  expect(await screen.findByText("Later title edit prevents Undo")).toBeVisible();
  expect(api.undoImportOperation).toHaveBeenCalledWith(expect.any(String),"old-operation");
 });
 it("blocked_optional_stage_preserves_successful_import",async()=>{
  const api=fixture();render(<ImportHistory api={api} onRefresh={()=>{}} onReview={()=>{}}/>);
  expect(await screen.findByText("Provider credentials are unavailable")).toBeVisible();
  expect(screen.getByText(/1 imported · 3 fields/)).toBeVisible();
  expect(screen.getByText(/1 proposed · 1 blocked/)).toBeVisible();
  expect(screen.getByRole("button",{name:"Recover effects"})).toBeDisabled();
  expect(screen.getByRole("button",{name:"Undo effects"})).toBeDisabled();
 });
 it("a racing local edit requires inspection again",async()=>{
  const api=fixture();api.previewImport=vi.fn(async()=>{throw {code:"stale_preview",message:"Library changed after preview"};});
  render(<LibraryImport api={api} initialSourceId="source-one" onClose={()=>{}}/>);
  await userEvent.click(await screen.findByRole("button",{name:"Processing choices"}));await userEvent.click(screen.getByRole("button",{name:"Review import"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("Library changed after preview");
  await userEvent.click(screen.getByRole("button",{name:"Inspect again"}));
  expect(screen.getByRole("button",{name:"Choose library"})).toBeVisible();
  expect(api.applyImport).not.toHaveBeenCalled();
 });
 it("a cancelled retry is not reported as successful processing",async()=>{
  const api=fixture();api.retryImportProcessing=vi.fn(async()=>{throw {code:"cancelled",message:"Operation cancelled"};});
  render(<ImportHistory api={api} onRefresh={()=>{}} onReview={()=>{}}/>);
  await userEvent.click(await screen.findByRole("button",{name:"Retry processing"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("Operation cancelled");
  expect(screen.queryByText(/processed ·.*reused/)).not.toBeInTheDocument();
  expect(api.retryImportProcessing).toHaveBeenCalledWith(expect.any(String),"old-operation");
 });
});
