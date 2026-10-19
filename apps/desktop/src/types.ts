export type View="tracks"|"incoming"|"archive";
export type WorkspaceView=View|"settings"|"cleanup";
export type Sort="title"|"artist"|"bpm"|"energy"|"added"|"playlist_order";
export interface Track {metadata:Record<string,unknown>;id:string;source_sha256:string;display_path:string;title:string;artist:string;album:string;genre:string;state:string;archived:boolean;key:string|null;bpm:number|null;energy:number|null;duration:number|null}
export interface TrackQuery {view:View;search:string;sort:Sort;descending:boolean;limit:number;cursor:string|null;playlist_id?:string|null}
export interface TrackPage {rows:Track[];total:number;revision:number;next_cursor:string|null}
export interface Cue {time_seconds:number;name:string;color:string|null;position?:number}
export interface PublishedCue {name:string;startTime:number;type:"1"|"5";position:number;color:string|null;endTime?:number;activeLoop?:boolean;data?:unknown}
export interface Publication {grid:{bpm:number;startTime:number};cues:PublishedCue[];implementation_sha256:string;inputs_sha256:string}
export interface Analysis {publication?:Publication;analysis:{duration_seconds:number;key:{camelot:string}|null;tempo:{bpm:number;beats_seconds:number[];downbeats_seconds:number[]|null}|null;energy:{level:number;method:string}|null;lexicon_cues:{cues:Cue[];emergency_loop:{time_seconds:number;loop_beats:number}|null}|null};media?:{source_sha256:string;pcm_sha256?:string;codec:string};source_sample_rate:number;complete:boolean}
export interface Detail {track:Track;analysis:Analysis|null;source_sha256:string;pcm_sha256:string|null}
export interface Job {id:string;track_id:string;title:string;state:string;stage:string;error:string|null;attempts:number}
export interface LexiconConfig {genre:string|null;drop_at_start:"Auto"|"Never";min_cue_length_beats:number;add_emergency_loop:boolean}
export interface EngineSettings {
  config:{key_frame_size:number;key_hop_size:number;rhythm_frame_size:number;rhythm_hop_size:number;min_bpm:number;max_bpm:number};
  sample_rate:number;max_seconds:number|null;beat_model:string|null;mel_model:string|null;
  key_model:string|null;key_neural_model:string|null;key_openkeyscan_model?:string|null;
  structure_features:boolean;source_profile_timing:boolean;lexicon_config:LexiconConfig|null;energy_model?:string|null;lexicon_energy?:boolean;
}
export interface TagFrame {id3:string;vorbis:string;mp4:string}
export interface TaggingOptions {
  platforms:string[];tags:string[];overwriteTags:string[];overwrite:boolean;
  threads:number;strictness:number;matchDuration:boolean;maxDurationDifference:number;matchById:boolean;
  multipleMatches:"Default"|"Oldest"|"Newest";multiplatform:boolean;shortTitle:boolean;
  parseFilename:boolean;filenameTemplate:string|null;titleRegex:string|null;skipTagged:boolean;
  includeSubfolders:boolean;albumTagging:boolean;albumTaggingRatio:number;
  camelot:boolean;mergeGenres:boolean;capitalizeGenres:boolean;stylesOptions:string;stylesCustomTag:TagFrame|null;
  albumArtFile:boolean;removeAllCovers:boolean;coverFilename:string|null;
  onlyYear:boolean;trackNumberLeadingZeroes:number;id3v24:boolean;id3CommLang:string|null;
  separators:{id3:string;vorbis:string|null;mp4:string};
  custom:Record<string,Record<string,unknown>>;
  [key:string]:unknown;
}
export interface AutotagConfig {options:TaggingOptions;no_match:"keep_existing"|"attention";analysis_tag_source:"measured"|"catalog";write_energy:boolean}
export type CustomOptionValue = {type:"boolean";value:boolean}|{type:"number";min:number;max:number;step:number;value:number}|{type:"string";value:string;hidden:boolean|null}|{type:"tag";value:TagFrame}|{type:"option";values:string[];value:string}|{type:"button"};
export interface TaggingProvider {id:string;name:string;maxThreads:number;supportedTags:string[];customOptions:{options:{id:string;label:string;tooltip:string|null;value:CustomOptionValue}[]}}
export interface TaggingCatalog {defaults:AutotagConfig;providers:TaggingProvider[]}
export type MetadataField="title"|"artist"|"album"|"album_artist"|"genre"|"comment"|"grouping"|"label"|"remixer"|"mix"|"producer"|"composer"|"lyricist";
export type ContributorRole="artist"|"remixer"|"producer"|"composer"|"lyricist";
export type CommonTextPattern="CamelotKeyFromTitle"|"OriginalMixFromTitle"|"CamelotKeyEnergyFromTitle"|"BpmFromTitle";
export type SmartFix={kind:"title_case";field:MetadataField;uppercase:boolean;lowercase:boolean}|{kind:"replace_characters";characters:string}|{kind:"decode_entities"}|{kind:"extract_artist";separator:string;part:number}|{kind:"extract_remixer";word:string;remove_title:boolean}|{kind:"remove_garbage";field:MetadataField}|{kind:"remove_number_prefix"}|{kind:"remove_urls";field:MetadataField;clear_field:boolean}|{kind:"mix_parentheses"}|{kind:"remove_common_text";pattern:CommonTextPattern}|{kind:"remove_promotional_text";field:MetadataField};
export interface ConfiguredFix {id:string;enabled:boolean;ignored_values:string[];rule:SmartFix}
export interface CanonicalMapping {from:string;to:string|null}
export interface GenreRules {enabled:boolean;mappings:CanonicalMapping[];protected_values:string[];conversion:{category:string;genres:string[];remove_genre:boolean}|null}
export interface ArtistRules {enabled:boolean;roles:ContributorRole[];mappings:CanonicalMapping[];protected_values:string[]}
export interface CleanupConfig {fixes:ConfiguredFix[];genres:GenreRules;artists:ArtistRules}
export interface HumanMetadata {fields:Partial<Record<MetadataField,string[]>>;custom_tags:{category:string;value:string}[]}
export interface FieldChange {field:MetadataField;before:string[];after:string[]}
export interface CleanupReport {original:HumanMetadata;metadata:HumanMetadata;changes:FieldChange[];applied_rules:string[]}
export interface CleanupFailure {track_id:string;message:string}
export interface CleanupPreview {id:string;created:number;proposals:{id:string;track_id:string;title:string;report:CleanupReport}[];unchanged:string[];failed:CleanupFailure[]}
export interface CleanupBatchResult {id:string;applied:string[];unchanged:string[];conflicted:CleanupFailure[];failed:CleanupFailure[]}
export interface CleanupHistory {can_undo:boolean;id:string;kind:string;state:string;created:number;result:CleanupBatchResult}
export interface CleanupCatalogValue {value:string;track_count:number;protected:boolean;mapping:CanonicalMapping|null}
export interface CleanupCatalog {genres:CleanupCatalogValue[];artists:(CleanupCatalogValue&{roles:Partial<Record<ContributorRole,number>>})[];categories:string[];unavailable_tracks:CleanupFailure[]}
export type CleanupScope={kind:"all_ready"}|{kind:"track_ids";track_ids:string[]};
export interface LibraryChangeEvent {track_ids:string[];playlist_ids?:string[]}
export interface Settings {cleanup?:CleanupConfig|null;tagging?:AutotagConfig|null;watch_folders:string[];stability_millis:number;engine:EngineSettings;autotag_enabled:boolean;cleanup_enabled:boolean}
export interface Snapshot {revision:string;settings:Settings;models:Record<string,string>}
export interface LibraryState {counts:{tracks:number;incoming:number;archive:number};revision:number;jobs:Job[];settings:Snapshot;service:{watch_errors:string[];last_error:string|null}}
export interface Playback {track_id:string|null;duration_seconds:number;position_seconds:number;playing:boolean;volume:number;error:string|null}
export interface PreparationEvent {sequence:number;attempt?:number;job_id:string;track_id:string;stage:string;state:string}
export interface PlaybackEvent {sequence:number;status:Playback}
export interface Waveform {duration_seconds:number;bins:{min:number;max:number}[];bands?:[number,number,number][]}
export interface Bridge {
  getPlaylists():Promise<import("./playlist-types").PlaylistWorkspace>;
  editPlaylist(edit:import("./playlist-types").PlaylistEdit,expectedRevision:number):Promise<import("./playlist-types").PlaylistChange>;
  undoPlaylist(editId:string,expectedRevision:number):Promise<import("./playlist-types").PlaylistChange>;
  getSmartCatalog():Promise<import("./playlist-types").SmartCatalog>;
  resolvePlaylistDate(day:string,expectedRevision:number):Promise<number>;
  previewSmartList(definition:import("./playlist-types").SmartDefinition,scope:{track_ids:string[]|null;playlist_id:string|null}):Promise<import("./playlist-types").SmartPreview>;
  saveSmartPlaylist(previewId:string,playlistId:string|null,name:string,parentId:string|null,expectedRevision:number):Promise<import("./playlist-types").PlaylistChange>;
  previewPlaylistTool(request:import("./playlist-types").PlaylistToolRequest):Promise<import("./playlist-types").PlaylistToolPreview>;
  applyPlaylistTool(previewId:string,expectedRevision:number):Promise<import("./playlist-types").PlaylistChange>;
  previewSmartGeneration(request:import("./playlist-types").SmartGenerationRequest):Promise<import("./playlist-types").GenerationPreview>;
  applySmartGeneration(previewId:string,expectedRevision:number):Promise<import("./playlist-types").PlaylistChange>;
  getPlaylistPreferences():Promise<import("./playlist-types").PlaylistPreferenceSnapshot>;
  savePlaylistPreferences(preferences:import("./playlist-types").PlaylistPreferences,expectedRevision:number):Promise<import("./playlist-types").PlaylistPreferenceSnapshot>;
  playlistMembers(playlistId:string,manual:boolean):Promise<string[]>;
  cleanupCatalog(config:CleanupConfig):Promise<CleanupCatalog>;
  previewCleanup(scope:CleanupScope,config:CleanupConfig):Promise<CleanupPreview>;
  applyCleanup(planId:string,proposalIds:string[]):Promise<CleanupBatchResult>;
  cleanupHistory():Promise<CleanupHistory[]>;
  undoCleanup(batchId:string):Promise<CleanupBatchResult>;
  onLibraryChange(handler:(event:LibraryChangeEvent)=>void):Promise<()=>void>;
  getState():Promise<LibraryState>;
  getMetadataFields():Promise<string[]>;
  getTaggingCatalog():Promise<TaggingCatalog>;
  getTaggingStorage():Promise<{original_files:number;original_bytes:number;pending_cleanup:number}>;
  listTracks(query:TrackQuery):Promise<TrackPage>;
  selectTrackIds(query:TrackQuery):Promise<string[]>;
  getTrack(id:string):Promise<Detail>;
  getArtwork(id:string,sourceSha256:string):Promise<string|null>;
  saveSettings(settings:Settings):Promise<Snapshot>;
  enqueuePaths(paths:string[]):Promise<string[]>;
  retryJob(id:string):Promise<void>;
  reprepareTrack(id:string):Promise<void>;
  archiveTrack(id:string,archived:boolean):Promise<void>;
  playerLoad(id:string):Promise<Detail>;
  playerPlay():Promise<void>;
  playerPause():Promise<void>;
  playerSeek(seconds:number):Promise<void>;
  playerVolume(volume:number):Promise<void>;
  playerStatus():Promise<Playback>;
  getWaveform(id:string,bins:number):Promise<Waveform>;
  onPreparation(handler:(event:PreparationEvent)=>void):Promise<()=>void>;
  onPlayback(handler:(event:PlaybackEvent)=>void):Promise<()=>void>;
  onDrop(handler:(paths:string[])=>void,active:(value:boolean)=>void):Promise<()=>void>;
  chooseFiles():Promise<string[]>;
  chooseFolder():Promise<string|null>;
}
