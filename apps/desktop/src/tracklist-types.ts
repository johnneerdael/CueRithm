import type {Track} from "./types";
import type {PlaylistChange} from "./playlist-types";
export interface TracklistOptions {format:"text"|"csv";headers:boolean;swap_artist_title:boolean;mode:"exact"|"fuzzy"}
export interface TracklistEntry {row:number;artist:string;title:string}
export interface TracklistCandidate {track:Track;reason:"same_tags"|"similar_tags";score:number}
export interface TracklistRow {entry:TracklistEntry;candidates:TracklistCandidate[];candidate_count:number}
export interface TracklistResult {rows:TracklistRow[];library_revision:number;playlist_revision:number;candidate_limit:number}
export interface TracklistSaveRequest {name:string;track_ids:string[];expected_library_revision:number;expected_playlist_revision:number}
export interface TracklistBridge {matchTracklist(input:string,options:TracklistOptions):Promise<TracklistResult>;saveTracklistPlaylist(request:TracklistSaveRequest):Promise<PlaylistChange>}
