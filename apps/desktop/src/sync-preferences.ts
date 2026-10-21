import {useCallback,useEffect,useState} from "react";
import type {SetStateAction} from "react";
import {defaultSyncOptions,syncTargets} from "./sync-types";
import type {DjSoftware,SyncOptions,SyncTextField} from "./sync-types";
import type {TargetEnvironment} from "./sync-target-types";

export const syncPreferencesKey="cuerithm-dj-sync-preferences-v1";
interface SyncChoices {path:string;sourceRoot:string|null;environment:TargetEnvironment;scope:"full"|"playlist"|"changed";playlistId:string;options:SyncOptions}
interface SyncPreferences {version:1;software:DjSoftware;profiles:Record<DjSoftware,SyncChoices>}
const fields:SyncTextField[]=["title","artist","album","genre","label","comment","key"];
const booleans=["engine_copy_audio","materialize_smartlists","usb_cleanup_unused","skip_beatgrids","nearest_color","add_album_art","extra_cue_markers"] as const;
function object(value:unknown):Record<string,unknown>{return value!==null&&typeof value==="object"&&!Array.isArray(value)?value as Record<string,unknown>:{};}
function choice<T extends string>(value:unknown,values:readonly T[],fallback:T):T{return typeof value==="string"&&values.includes(value as T)?value as T:fallback;}
function text(value:unknown):string{return typeof value==="string"&&!value.includes("\0")?value:"";}
function defaults():SyncChoices{return {path:"",sourceRoot:null,environment:{kind:"native"},scope:"full",playlistId:"",options:{...defaultSyncOptions,field_mappings:{}}};}
function environment(value:unknown):TargetEnvironment{const raw=object(value);return raw.kind==="windows_compatibility"?{kind:"windows_compatibility",prefix:text(raw.prefix),installation:text(raw.installation)||null,mappings:Array.isArray(raw.mappings)?raw.mappings.slice(0,64).map(value=>{const row=object(value);return {windows_root:text(row.windows_root),host_root:text(row.host_root)};}):[]}:{kind:"native"};}
function profile(value:unknown):SyncChoices{
 const saved=object(value),raw=object(saved.options),options=defaults().options;
 for(const key of booleans)if(typeof raw[key]==="boolean")options[key]=raw[key];
 options.key_notation=choice(raw.key_notation,["original","musical","camelot","open_key"],options.key_notation);
 options.usb_cue_destination=choice(raw.usb_cue_destination,["hot","memory","both"],options.usb_cue_destination);
 options.usb_cdj_compatibility=choice(raw.usb_cdj_compatibility,["off","detect","auto_fix"],options.usb_cdj_compatibility);
 options.beatgrid_lock=choice(raw.beatgrid_lock,["default","lock","unlock"],options.beatgrid_lock);
 const mappings=object(raw.field_mappings);
 for(const field of fields)if(typeof mappings[field]==="string"&&fields.includes(mappings[field] as SyncTextField))options.field_mappings[field]=mappings[field] as SyncTextField;
 return {path:text(saved.path),sourceRoot:text(saved.sourceRoot)||null,environment:environment(saved.environment),scope:choice(saved.scope,["full","playlist","changed"],"full"),playlistId:text(saved.playlistId),options};
}
export function readSyncPreferences():SyncPreferences{
 let saved:Record<string,unknown>={};
 try{const value=object(JSON.parse(localStorage.getItem(syncPreferencesKey)??"null"));if(value.version===1)saved=value;}catch{}
 const profiles=object(saved.profiles);
 return {version:1,software:choice(saved.software,syncTargets.map(target=>target.id),"virtual_dj"),profiles:Object.fromEntries(syncTargets.map(target=>[target.id,profile(profiles[target.id])])) as Record<DjSoftware,SyncChoices>};
}
/** Persist editable choices only. Prepared journals and operation state stay in Sync. */
export function useSyncPreferences(){
 const [preferences,setPreferences]=useState(readSyncPreferences);
 useEffect(()=>{try{localStorage.setItem(syncPreferencesKey,JSON.stringify(preferences));}catch{}},[preferences]);
 const setSoftware=useCallback((software:DjSoftware)=>setPreferences(previous=>({...previous,software})),[]);
 const setChoices=useCallback((update:SetStateAction<SyncChoices>)=>setPreferences(previous=>{
  const current=previous.profiles[previous.software],next=typeof update==="function"?update(current):update;
  return {...previous,profiles:{...previous.profiles,[previous.software]:next}};
 }),[]);
 return {software:preferences.software,choices:preferences.profiles[preferences.software],setSoftware,setChoices};
}
