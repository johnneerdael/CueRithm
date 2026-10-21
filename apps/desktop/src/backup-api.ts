import {invoke} from "@tauri-apps/api/core";
import {open} from "@tauri-apps/plugin-dialog";
import type {LibraryBackupBridge} from "./backup-types";
export const nativeLibraryBackup:LibraryBackupBridge={
 chooseFolder:async title=>{const path=await open({title,directory:true,multiple:false});return typeof path==="string"?path:null;},
 create:destination=>invoke("create_library_backup",{destination}),
 inspect:bundle=>invoke("inspect_library_backup",{bundle}),
 stage:(bundle,sha256)=>invoke("stage_library_restore",{bundle,sha256}),
 pending:()=>invoke("pending_library_restore"),
 retry:()=>invoke("retry_library_restore"),
 cancel:()=>invoke("cancel_library_restore"),
};
