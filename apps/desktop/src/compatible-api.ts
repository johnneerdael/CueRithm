import {invoke} from "@tauri-apps/api/core";
import type {CompatibleBridge} from "./compatible-types";
export const nativeCompatibleBridge:CompatibleBridge={saveCompatiblePlaylist:request=>invoke("save_compatible_playlist",{request}),findCompatibleTracks:request=>invoke("find_compatible_tracks",{request})};
