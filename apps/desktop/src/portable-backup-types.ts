export interface PortableRoot {id:string;label:string;source_path:string;track_count:number}
export interface PortableInspection {preview_id:string;source_format:"cuerithm"|"lexicon";source_profile:string;created:number|null;tracks:number;playlists:number;cues:number;roots:PortableRoot[];warnings:string[];losses:string[];source_digest:string}
export interface PortableGeneration {id:string;created:number;tracks:number}
export interface PortableGenerations {active:string;retained:PortableGeneration[]}
export interface PortableRestoreRequest {preview_id:string;source_digest:string;root_mappings:{root_id:string;destination:string|null}[];acknowledge_losses:boolean;replace_generation:string}
export interface PortableRestoreResult {active_generation:string;retained_generation:string;tracks:number;unresolved_tracks:number;warnings:string[];restart_required:boolean}
export interface PortableExportResult {path:string;tracks:number;size_bytes:number}
export interface PortableProgress {task_id:string;phase:string;completed:number;total:number|null;bytes:number}
export interface PortableBackupBridge {
 chooseArchive:()=>Promise<string|null>;chooseDestination:()=>Promise<string|null>;chooseFolder:(title:string)=>Promise<string|null>;
 export:(operationId:string,destination:string,includeHistory:boolean)=>Promise<PortableExportResult>;
 inspect:(operationId:string,path:string,importTimeZone:string)=>Promise<PortableInspection>;
 restore:(operationId:string,request:PortableRestoreRequest)=>Promise<PortableRestoreResult>;
 generations:()=>Promise<PortableGenerations>;
 activate:(operationId:string,generationId:string,expectedActive:string)=>Promise<PortableRestoreResult>;
 cancel:(operationId:string)=>Promise<boolean>;
 onProgress:(handler:(progress:PortableProgress)=>void)=>Promise<()=>void>;
}
