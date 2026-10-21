import type {Track} from "./types";
export interface CompatibleTrackRequest {source_id:string;compatible_key:boolean;key_mode:"harmonic"|"fuzzy"|null;bpm_tolerance:number|null;include_half_double:boolean;energy_tolerance:number|null;genre:string|null;custom_tag_ids:string[]}
export interface CompatibleTrackMatch {track:Track;reasons:string[];bpm_delta:number|null;bpm_multiplier:number|null;energy_delta:number|null;same_key:boolean}
export interface CompatibleTrackResult {source:Track;key_mode:"harmonic"|"fuzzy";matches:CompatibleTrackMatch[];checked:number;excluded_unknown:number;library_revision:number}
export interface CompatiblePlaylistSaveRequest {name:string;track_ids:string[];expected_library_revision:number;expected_playlist_revision:number}
export interface CompatibleBridge {saveCompatiblePlaylist(request:CompatiblePlaylistSaveRequest):Promise<import("./playlist-types").PlaylistChange>;findCompatibleTracks(request:CompatibleTrackRequest):Promise<CompatibleTrackResult>}
