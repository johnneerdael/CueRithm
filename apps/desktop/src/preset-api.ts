import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import type {PreparationPresetsBridge} from "./preset-types";
export const nativePreparationPresets:PreparationPresetsBridge={
 preparationPresets:()=>invoke("preparation_presets"),preparationPreset:id=>invoke("preparation_preset",{id}),
 savePreparationPreset:request=>invoke("save_preparation_preset",{request}),
 duplicatePreparationPreset:(id,revision,name)=>invoke("duplicate_preparation_preset",{id,revision,name}),deletePreparationPreset:(id,revision)=>invoke("delete_preparation_preset",{id,revision}),
 previewPreparationPresetImport:document=>invoke("preview_preparation_preset_import",{document}),importPreparationPreset:(document,nameOverride)=>invoke("import_preparation_preset",{document,nameOverride}),exportPreparationPreset:(id,revision)=>invoke("export_preparation_preset",{id,revision}),
 projectPreparationPreset:settings=>invoke("project_preparation_preset",{settings}),overlayPreparationPreset:(settings,id,revision)=>invoke("overlay_preparation_preset",{settings,id,revision}),preparationPresetFolders:settings=>invoke("preparation_preset_folders",{settings}),
 choosePreparationPresetImport:()=>invoke("choose_preparation_preset_import"),savePreparationPresetExport:(id,revision)=>invoke("save_preparation_preset_export",{id,revision}),onPreparationPresetsChange:handler=>listen("preparation-presets-change",()=>handler()),
};
