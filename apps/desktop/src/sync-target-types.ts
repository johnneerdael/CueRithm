import type {SyncTarget} from "./sync-types";
import type {RekordboxSyncTarget} from "./rekordbox-sync-types";
import {pathKey} from "./native-paths";
export type TargetEnvironment={kind:"native"}|{kind:"windows_compatibility";prefix:string;installation:string|null;mappings:{windows_root:string;host_root:string}[]};
export interface SyncConfigurationBinding {id:string;revision:number}
export type SavedSyncTarget={kind:"dj_library";target:SyncTarget}|{kind:"rekordbox";target:RekordboxSyncTarget};
export interface SyncTargetConfiguration {id:string;name:string;revision:number;target:SavedSyncTarget}
export interface SaveSyncTargetConfiguration {id:string|null;expected_revision:number|null;name:string;target:SavedSyncTarget}
export interface SyncConfigurationBridge {
 syncTargetConfigurations():Promise<SyncTargetConfiguration[]>;
 saveSyncTargetConfiguration(request:SaveSyncTargetConfiguration):Promise<SyncTargetConfiguration>;
 deleteSyncTargetConfiguration(id:string,expectedRevision:number):Promise<void>;
}
export function sameDjTarget(a:SyncTarget,b:SyncTarget):boolean{
 const key=(target:SyncTarget)=>{const environment=target.environment;return JSON.stringify([target.software,pathKey(target.path),target.source_root===null?null:pathKey(target.source_root),environment?.kind==="windows_compatibility"?[environment.prefix,environment.installation,environment.mappings.map(mapping=>[mapping.windows_root,mapping.host_root])]:null]);};
 return key(a)===key(b);
}
