import {createContext,useContext} from "react";
import type {MaintenanceBridge} from "../maintenance-api";
import type {DuplicateSkippedTrack,MaintenanceProgress} from "../maintenance-types";
import type {MissingRepairBridge} from "../missing-repair-api";
export type TaskKind=MaintenanceProgress["kind"];
export type MaintenanceTab="duplicates"|"broken"|"unused"|"history";
export interface MaintenanceTask {api:MaintenanceBridge;repair:MissingRepairBridge;busy:boolean;externalChange:number;historyVersion:number;run:<T>(kind:TaskKind,label:string,work:(id:string)=>Promise<T>)=>Promise<T|undefined>;changed:()=>void;message:(value:string)=>void;openTab:(tab:MaintenanceTab)=>void}
export const MaintenanceContext=createContext<MaintenanceTask|null>(null);
export function useMaintenance(){const value=useContext(MaintenanceContext);if(!value)throw new Error("Maintenance context missing");return value;}
export function bytes(value:number){return value<1024?`${value} B`:value<1048576?`${(value/1024).toFixed(1)} KB`:value<1073741824?`${(value/1048576).toFixed(1)} MB`:`${(value/1073741824).toFixed(2)} GB`;}
export function savedId(key:string){try{return localStorage.getItem("cuerithm-maintenance-"+key)??"";}catch{return "";}}
export function saveId(key:string,value:string){try{localStorage.setItem("cuerithm-maintenance-"+key,value);}catch{}}
export function Stale({stale,reason}:{stale:boolean;reason:string|null}){return stale?<p role="alert" className="maintenance-warning">{reason??"Inputs changed. Rescan before continuing."}</p>:null;}
export function toggle(set:Set<string>,id:string){const next=new Set(set);if(next.has(id))next.delete(id);else next.add(id);return next;}
/** Plain-language reason a duplicate scan left a track out. The report decides which tracks are skipped; this only words it. */
export function skippedNotice(skipped:DuplicateSkippedTrack[]){const reason=(state:string)=>state==="missing"?"file not found":state==="unavailable"?"location not available":"file cannot be read";const found=new Set<string>(skipped.map(t=>reason(t.state)));const words=["file not found","location not available","file cannot be read"].filter(w=>found.has(w));const many=skipped.length!==1;return `${skipped.length} ${many?"tracks":"track"} skipped: ${words.join(", ")}. Review ${many?"them":"it"} in Missing & broken.`;}
export const plural=(n:number,one:string,many:string)=>`${n} ${n===1?one:many}`;
/** The one Undo bound. Duplicate merges that delete files and Missing & broken removals share it. */
export const undoLimitNote=(limit:number)=>`Undo is kept for your last ${limit} removals of library records; duplicate merges that delete files and Missing & broken removals count together. After that a removal is final.`;
/** Why a "Delete from disk" merge leaves a file exactly where it is. */
export const skipReasons:Record<string,string>={not_verified_identical:"The audio is not proven identical to the kept track.",file_changed:"The file changed after the scan.",file_unavailable:"The file is not available.",shared_with_remaining_record:"Another library track still uses this file.",same_file_as_keeper:"This is the kept track’s own file.",busy:"The track or file is in use by another operation.",trash_failed:"The Trash could not take the file."};
export const fileStates:Record<string,string>={present:"Present",missing:"Missing",unavailable:"Unavailable",unsupported:"Unsupported",timed_out:"Timed out",decode_error:"Decode error",symlink:"Symbolic link",not_regular:"Not a regular file",changed:"Changed",video:"Video",no_audio:"No audio",duration_skipped:"Duration skipped"};
