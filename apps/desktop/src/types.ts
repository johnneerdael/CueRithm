export type View="tracks"|"incoming"|"archive";
export type Sort="title"|"artist"|"bpm"|"energy"|"added";
export interface Track {metadata:Record<string,unknown>;id:string;source_sha256:string;display_path:string;title:string;artist:string;album:string;genre:string;state:string;key:string|null;bpm:number|null;energy:number|null;duration:number|null}
export interface TrackQuery {view:View;search:string;sort:Sort;descending:boolean;limit:number;cursor:string|null}
export interface TrackPage {rows:Track[];total:number;revision:number;next_cursor:string|null}
export interface Cue {time_seconds:number;name:string;color:string}
export interface Analysis {analysis:{duration_seconds:number;key:{camelot:string}|null;tempo:{bpm:number;beats_seconds:number[];downbeats_seconds:number[]|null}|null;energy:{level:number;method:string}|null;lexicon_cues:{cues:Cue[];emergency_loop:{time_seconds:number;loop_beats:number}|null}|null};media?:{source_sha256:string;pcm_sha256?:string;codec:string};source_sample_rate:number;complete:boolean}
export interface Detail {track:Track;analysis:Analysis|null;source_sha256:string;pcm_sha256:string|null}
export interface Job {id:string;track_id:string;title:string;state:string;stage:string;error:string|null;attempts:number}
export interface LexiconConfig {genre:string|null;drop_at_start:"Auto"|"Never";min_cue_length_beats:number;add_emergency_loop:boolean}
export interface EngineSettings {
  config:{key_frame_size:number;key_hop_size:number;rhythm_frame_size:number;rhythm_hop_size:number;min_bpm:number;max_bpm:number};
  sample_rate:number;max_seconds:number|null;beat_model:string|null;mel_model:string|null;
  key_model:string|null;key_neural_model:string|null;key_openkeyscan_model?:string|null;
  structure_features:boolean;source_profile_timing:boolean;lexicon_config:LexiconConfig|null;energy_model?:string|null;
}
export interface Settings {watch_folders:string[];stability_millis:number;engine:EngineSettings;autotag_enabled:boolean;cleanup_enabled:boolean}
export interface Snapshot {revision:string;settings:Settings;models:Record<string,string>}
export interface LibraryState {counts:{tracks:number;incoming:number;archive:number};revision:number;jobs:Job[];settings:Snapshot;service:{watch_errors:string[];last_error:string|null}}
export interface Playback {track_id:string|null;duration_seconds:number;position_seconds:number;playing:boolean;volume:number;error:string|null}
export interface PreparationEvent {sequence:number;attempt?:number;job_id:string;track_id:string;stage:string;state:string}
export interface PlaybackEvent {sequence:number;status:Playback}
export interface Waveform {duration_seconds:number;bins:{min:number;max:number}[]}
export interface Bridge {
  getState():Promise<LibraryState>;
  getMetadataFields():Promise<string[]>;
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
