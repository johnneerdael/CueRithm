import {invoke} from "@tauri-apps/api/core";
import type * as T from "./import-types";
export const importApi={
 discoverImportSources:()=>invoke<T.ImportSourceCandidate[]>("discover_import_sources"),
 chooseImportSource:(operationId:string,provider:T.SourceProvider)=>invoke<T.NativeImportSelection|null>("choose_import_source",{operationId,provider}),
 chooseRekordboxImportSource:(operationId:string,targetId:string)=>invoke<T.NativeImportSelection>("choose_rekordbox_import_source",{operationId,targetId}),
 addImportArtifacts:(operationId:string,selectionId:string)=>invoke<T.NativeImportSelection|null>("add_import_artifacts",{operationId,selectionId}),
 discoverRekordboxSources:()=>invoke<import("./rekordbox-sync-types").RekordboxSyncTarget[]>("discover_rekordbox_targets"),
 chooseImportMapping:()=>invoke<{root_id:string;display_location:string}|null>("choose_import_mapping"),
 inspectImport:(operationId:string,request:T.ImportInspectRequest)=>invoke<T.ImportInspection>("inspect_import",{operationId,request}),
 importPage:(inspectionId:string,query:T.ImportPageQuery)=>invoke<T.ImportPage>("import_page",{inspectionId,query}),
 previewImport:(operationId:string,request:T.ImportPreviewRequest)=>invoke<T.ImportPreview>("preview_import",{operationId,request}),
 importPreviewPage:(previewId:string,query:T.PageQuery={})=>invoke<T.ImportPreviewPage>("import_preview_page",{previewId,query}),
 applyImport:(operationId:string,previewId:string)=>invoke<T.ImportApplyResult>("apply_import",{operationId,previewId}),
 discardImportPreview:(previewId:string)=>invoke<void>("discard_import_preview",{previewId}),
 inspectImportRefresh:(operationId:string,sourceId:string)=>invoke<T.ImportInspection>("inspect_import_refresh",{operationId,sourceId}),
 importOperationPage:(query:T.ImportOperationQuery={})=>invoke<T.ImportOperationPage>("import_operation_page",{query}),
 previewImportEffects:(operationId:string,id:string)=>invoke<T.ImportEffectsPreview>("preview_import_effects",{operationId,id}),
 importEffectsPage:(previewId:string,query:T.PageQuery={})=>invoke<T.ImportEffectsPage>("import_effects_page",{previewId,query}),
 applyImportEffects:(operationId:string,previewId:string)=>invoke<T.ImportProcessingSummary>("apply_import_effects",{operationId,previewId}),
 recoverImportOperation:(operationId:string,id:string)=>invoke<T.ImportRecoveryResult>("recover_import_operation",{operationId,id}),
 undoImportOperation:(operationId:string,id:string)=>invoke<T.ImportUndoResult>("undo_import_operation",{operationId,id}),
 importedItem:(id:string)=>invoke<T.ImportedItemDetail>("imported_item",{id}),
 importedCollectionPage:(id:string,query:T.PageQuery={})=>invoke<T.ImportedCollectionPage>("imported_collection_page",{id,query}),
 runImportProcessing:(operationId:string)=>invoke<T.ImportProcessingSummary>("run_import_processing",{operationId}),
 cancel:(operationId:string)=>invoke<boolean>("cancel_organization",{operationId}),
};
export type ImportBridge=typeof importApi;
