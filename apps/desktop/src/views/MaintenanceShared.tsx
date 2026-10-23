import {createContext,useContext} from "react";
import type {MaintenanceBridge} from "../maintenance-api";
import type {MaintenanceProgress} from "../maintenance-types";
import type {MissingRepairBridge} from "../missing-repair-api";
export type TaskKind=MaintenanceProgress["kind"];
export interface MaintenanceTask {api:MaintenanceBridge;repair:MissingRepairBridge;busy:boolean;externalChange:number;historyVersion:number;run:<T>(kind:TaskKind,label:string,work:(id:string)=>Promise<T>)=>Promise<T|undefined>;changed:()=>void;message:(value:string)=>void}
export const MaintenanceContext=createContext<MaintenanceTask|null>(null);
export function useMaintenance(){const value=useContext(MaintenanceContext);if(!value)throw new Error("Maintenance context missing");return value;}
export function bytes(value:number){return value<1024?`${value} B`:value<1048576?`${(value/1024).toFixed(1)} KB`:value<1073741824?`${(value/1048576).toFixed(1)} MB`:`${(value/1073741824).toFixed(2)} GB`;}
export function savedId(key:string){try{return localStorage.getItem("cuerithm-maintenance-"+key)??"";}catch{return "";}}
export function saveId(key:string,value:string){try{localStorage.setItem("cuerithm-maintenance-"+key,value);}catch{}}
export function Stale({stale,reason}:{stale:boolean;reason:string|null}){return stale?<p role="alert" className="maintenance-warning">{reason??"Inputs changed. Rescan before continuing."}</p>:null;}
export function toggle(set:Set<string>,id:string){const next=new Set(set);if(next.has(id))next.delete(id);else next.add(id);return next;}
export const fileStates:Record<string,string>={present:"Present",missing:"Missing",unavailable:"Unavailable",unsupported:"Unsupported",timed_out:"Timed out",decode_error:"Decode error",symlink:"Symbolic link",not_regular:"Not a regular file",changed:"Changed",video:"Video",no_audio:"No audio",duration_skipped:"Duration skipped"};
