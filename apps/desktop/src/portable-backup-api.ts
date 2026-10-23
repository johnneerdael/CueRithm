import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {open,save} from "@tauri-apps/plugin-dialog";
import type {PortableBackupBridge,PortableProgress} from "./portable-backup-types";
export const nativePortableBackup:PortableBackupBridge={
 chooseArchive:async()=>{const value=await open({title:"Inspect a CueRithm or Lexicon backup",multiple:false,filters:[{name:"Library backup",extensions:["cuerithm","zip"]},{name:"All files",extensions:["*"]}]});return typeof value==="string"?value:null;},
 chooseDestination:()=>save({title:"Export portable library",defaultPath:"CueRithm.cuerithm",filters:[{name:"CueRithm library",extensions:["cuerithm"]}]}),
 chooseFolder:async title=>{const value=await open({title,directory:true,multiple:false});return typeof value==="string"?value:null;},
 export:(operationId,destination,includeHistory)=>invoke("portable_export",{taskId:operationId,destination,includeHistory}),
 inspect:(operationId,path,importTimeZone)=>invoke("portable_inspect",{taskId:operationId,path,importTimeZone}),
 restore:(operationId,request)=>invoke("portable_restore",{taskId:operationId,request}),
 generations:()=>invoke("portable_generations"),
 activate:(operationId,generationId,expectedActive)=>invoke("portable_activate_generation",{taskId:operationId,generationId,expectedActive}),
 cancel:operationId=>invoke("portable_cancel",{taskId:operationId}),
 onProgress:handler=>listen<PortableProgress>("portable-progress",event=>handler(event.payload)),
};
