import {beforeEach, describe, expect, it, vi} from "vitest";
const native=vi.hoisted(()=>({invoke:vi.fn(async()=>undefined)}));
vi.mock("@tauri-apps/api/core",()=>({invoke:native.invoke}));
import {importApi} from "../import-api";
import {defaultImportPolicy} from "../import-types";
import contract from "../../../../docs/development/fixtures/library-import-ipc.json";
describe("library import native boundary",()=>{
 beforeEach(()=>native.invoke.mockClear());
 it("opaque_preview_id_is_the_only_apply_authority",async()=>{
  await importApi.applyImport("work-1","server-preview");
  expect(native.invoke).toHaveBeenCalledExactlyOnceWith("apply_import",{operationId:"work-1",previewId:"server-preview"});
  await importApi.applyImportEffects("work-2","effects-preview");
  expect(native.invoke).toHaveBeenLastCalledWith("apply_import_effects",{operationId:"work-2",previewId:"effects-preview"});
 });
 it("ipc_contract_matches_rust_fixture",()=>{
  expect(defaultImportPolicy()).toEqual(contract.policy);
  expect(contract.presence).toEqual({presence:"value",value:"Invented title"});
  expect(contract.capability).toEqual({read:true,write:false,lossless:true,evidence:"d2",reason:null});
  expect(contract.clock_status).toBe("unresolved_native");
 });
 it("forwards structured errors without flattening their codes",async()=>{
  for(const code of ["stale_preview","unsupported_profile","source_busy","recovery_required"]){
   native.invoke.mockRejectedValueOnce({code,message:"Review the source again"});
   await expect(importApi.applyImport("work","preview")).rejects.toEqual({code,message:"Review the source again"});
  }
 });
 it("selection and cancellation use opaque IDs",async()=>{
  await importApi.inspectImport("scan",{selection_id:"native-choice",binding:{choice:"automatic"},capture_file_tags:false,include_history:false,include_queue:false,path_mappings:[]});
  expect(native.invoke).toHaveBeenLastCalledWith("inspect_import",{operationId:"scan",request:expect.objectContaining({selection_id:"native-choice"})});
  await importApi.cancel("scan");
  expect(native.invoke).toHaveBeenLastCalledWith("cancel_organization",{operationId:"scan"});
  await importApi.retryImportProcessing("retry-work","durable-operation");
  expect(native.invoke).toHaveBeenLastCalledWith("retry_import_processing",{operationId:"retry-work",id:"durable-operation"});
 });
});
