import {invoke} from "@tauri-apps/api/core";
import type {ManualPublicationApi} from "./manual-publication-types";
export const manualPublicationApi:ManualPublicationApi={
  snapshot:trackId=>invoke("manual_publication_snapshot",{trackId}),
  preview:request=>invoke("preview_manual_publication",{request}),
  apply:(previewId,expectedRevision)=>invoke("apply_manual_publication",{previewId,expectedRevision}),
  history:trackId=>invoke("manual_publication_history",{trackId}),
  undo:(editId,expectedRevision)=>invoke("undo_manual_publication",{editId,expectedRevision}),
  templates:()=>invoke("list_cue_templates"),
  saveTemplate:(id,name,template,expectedRevision)=>invoke("save_cue_template",{id,name,template,expectedRevision}),
  deleteTemplate:(id,expectedRevision)=>invoke("delete_cue_template",{id,expectedRevision}),
  previewTemplate:request=>invoke("preview_cue_template",{request}),
};
