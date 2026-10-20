import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import type {MetadataEditorBridge} from "./editor-types";
export const nativeMetadataEditor:MetadataEditorBridge={
  onMetadataEditChange:handler=>listen("metadata-edit-change",()=>handler()),
  metadataEditorSnapshot:(operationId,trackIds)=>invoke("metadata_editor_snapshot",{operationId,trackIds}),
  createMetadataDraft:(operationId,trackIds)=>invoke("create_metadata_draft",{operationId,trackIds}),
  metadataDraft:id=>invoke("metadata_draft",{id}),
  metadataDrafts:()=>invoke("metadata_drafts"),
  updateMetadataDraft:(id,expectedRevision,patch)=>invoke("update_metadata_draft",{id,expectedRevision,patch}),
  previewMetadataEdits:(operationId,id,revision)=>invoke("preview_metadata_edits",{operationId,id,revision}),
  applyMetadataEdits:(operationId,previewId)=>invoke("apply_metadata_edits",{operationId,previewId}),
  recoverMetadataEdits:(operationId,batchId)=>invoke("recover_metadata_edits",{operationId,batchId}),
  undoMetadataEdits:(operationId,batchId)=>invoke("undo_metadata_edits",{operationId,batchId}),
  discardMetadataDraft:(id,revision)=>invoke("discard_metadata_draft",{id,revision}),
  metadataEditHistoryPage:(offset,limit)=>invoke("metadata_edit_history_page",{offset,limit}),
  metadataEditBatch:id=>invoke("metadata_edit_batch",{id}),
  chooseMetadataArtwork:()=>invoke("choose_metadata_artwork"),
  extractMetadataArtwork:(operationId,trackId,sourceSha256,identity)=>invoke("extract_metadata_artwork",{operationId,trackId,sourceSha256,identity}),
  metadataArtworkAsset:assetId=>invoke("metadata_artwork_asset",{assetId}),
  cancelMetadataOperation:operationId=>invoke("maintenance_cancel",{operationId}),
};
