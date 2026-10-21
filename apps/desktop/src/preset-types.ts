import type {Settings,LexiconConfig,TaggingOptions,CleanupConfig} from "./types";
import type {OrganizationSettings} from "./organization-types";
export interface PresetIssue{code:string;field:string;message:string}
export type PresetTaggingOptions=Pick<TaggingOptions,"platforms"|"tags"|"separators"|"id3v24"|"overwrite"|"overwriteTags"|"strictness"|"mergeGenres"|"albumArtFile"|"camelot"|"parseFilename"|"filenameTemplate"|"shortTitle"|"matchDuration"|"maxDurationDifference"|"matchById"|"multipleMatches"|"stylesOptions"|"stylesCustomTag"|"trackNumberLeadingZeroes"|"skipTagged"|"onlyYear"|"titleRegex"|"capitalizeGenres"|"id3CommLang"|"removeAllCovers"|"multiplatform"|"albumTagging"|"albumTaggingRatio"|"coverFilename">&{fetchAllResults:boolean;custom:{beatport:{max_pages:number;art_resolution:number;ignore_version:boolean}|null;traxsource:Record<string,never>|null}};
export interface PreparationPresetRules{cue_generation:LexiconConfig;autotag_enabled:boolean;tagging:{options:PresetTaggingOptions;no_match:string;analysis_tag_source:string;write_energy:boolean;required_tags:string[]}|null;cleanup_enabled:boolean;cleanup:CleanupConfig|null;organization:Pick<OrganizationSettings,"mode"|"pattern"|"missing"|"collision"|"organization_optional">}
export interface PresetConnections{destination_root_id:string|null}
export interface PreparationPresetSummary{id:string;revision:string;name:string;status:string;issues:PresetIssue[];assigned_folder_count:number}
export interface PreparationPreset extends PreparationPresetSummary{rules:PreparationPresetRules;connections:PresetConnections}
export interface PreparationPresetProjection{rules:PreparationPresetRules;connections:PresetConnections}
export interface PreparationPresetSave extends PreparationPresetProjection{id:string|null;expected_revision:string|null;name:string}
export interface PreparationPresetImportPreview{name:string;rules:PreparationPresetRules;status:string;issues:PresetIssue[]}
export interface PreparationPresetExport{document:string;warnings:PresetIssue[]}
export type PreparationFolderChoice={kind:"inherit"}|{kind:"global"}|{kind:"preset";preset_id:string};
export interface PreparationFolderChange{folder_id:string;expected_revision:string|null;choice:PreparationFolderChoice}
export interface PreparationFolderAssignment{folder_id:string;display_path:string;revision:string|null;choice:PreparationFolderChoice;effective_preset_id:string|null;effective_preset_revision:string|null;matched_folder_id:string|null;status:string;issues:PresetIssue[]}
export interface PreparationPresetsBridge{
 preparationPresets():Promise<PreparationPresetSummary[]>;
 preparationPreset(id:string):Promise<PreparationPreset>;
 savePreparationPreset(request:PreparationPresetSave):Promise<PreparationPreset>;
 duplicatePreparationPreset(id:string,revision:string,name:string):Promise<PreparationPreset>;
 deletePreparationPreset(id:string,revision:string):Promise<void>;
 previewPreparationPresetImport(document:string):Promise<PreparationPresetImportPreview>;
 importPreparationPreset(document:string,nameOverride:string|null):Promise<PreparationPreset>;
 exportPreparationPreset(id:string,revision:string):Promise<PreparationPresetExport>;
 projectPreparationPreset(settings:Settings):Promise<PreparationPresetProjection>;
 overlayPreparationPreset(settings:Settings,id:string,revision:string):Promise<Settings>;
 preparationPresetFolders(settings:Settings):Promise<PreparationFolderAssignment[]>;
 choosePreparationPresetImport():Promise<string|null>;
 savePreparationPresetExport(id:string,revision:string):Promise<boolean>;
 onPreparationPresetsChange(handler:()=>void):Promise<()=>void>;
}
