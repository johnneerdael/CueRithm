import {invoke} from "@tauri-apps/api/core";
import type {TracklistBridge} from "./tracklist-types";
export const nativeTracklistBridge:TracklistBridge={matchTracklist:(input,options)=>invoke("match_tracklist",{input,options}),saveTracklistPlaylist:request=>invoke("save_tracklist_playlist",{request})};
