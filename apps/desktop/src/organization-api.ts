import {invoke} from "@tauri-apps/api/core";
import type {OrganizationBridge} from "./organization-types";
export const nativeOrganization:OrganizationBridge={
  organizationRoots:()=>invoke("organization_roots"),
  chooseOrganizationRoot:()=>invoke("choose_organization_root"),
  organizationExample:settings=>invoke("organization_example",{settings}),
  previewOrganization:(operationId,request)=>invoke("preview_organization",{operationId,request}),
  organizationPreviewPage:(id,offset,limit)=>invoke("organization_preview_page",{id,offset,limit}),
  discardOrganizationPreview:id=>invoke("discard_organization_preview",{id}),
  applyOrganization:(operationId,previewId,revision)=>invoke("apply_organization",{operationId,previewId,revision}),
  cancelOrganization:operationId=>invoke("cancel_organization",{operationId}),
  cancelOrganizationBatch:id=>invoke("cancel_organization_batch",{id}),
  organizationHistory:()=>invoke("organization_history"),
  organizationBatchPage:(id,offset,limit)=>invoke("organization_batch_page",{id,offset,limit}),
  recoverOrganization:(operationId,id)=>invoke("recover_organization",{operationId,id}),
  undoOrganization:(operationId,id)=>invoke("undo_organization",{operationId,id}),
  organizationAlternates:()=>invoke("organization_alternates"),
  resolveOrganizationAlternate:(id,resolution,operationId)=>invoke("resolve_organization_alternate",{id,resolution,operationId}),
};
