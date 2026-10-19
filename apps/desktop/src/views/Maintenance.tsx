import {useEffect,useState} from "react";
import {ChevronLeft} from "lucide-react";
import type {Bridge} from "../types";
import type {MaintenanceBridge} from "../maintenance-api";
import {useMaintenanceTask} from "../use-maintenance-task";
import DuplicatesTab from "./maintenance/DuplicatesTab";
import BrokenTab from "./maintenance/BrokenTab";
import UnusedTab from "./maintenance/UnusedTab";
import BackupsTab from "./maintenance/BackupsTab";

const tabs=[["duplicates","Duplicates"],["broken","Missing files"],["unused","Unused files"],["backups","Retained backups"]] as const;
type Tab=typeof tabs[number][0];

/**
 * Library maintenance: duplicate review and merge, missing/broken file checks, reviewed unused-file removal and
 * backup disposal. One native maintenance operation runs at a time, so the tabs share a single task slot.
 */
export default function Maintenance({bridge,maintenance,onLibraryChanged,onBack}:{bridge:Bridge;maintenance:MaintenanceBridge;onLibraryChanged:()=>void;onBack:()=>void}){
  const [tab,setTab]=useState<Tab>("duplicates"),[changeTick,setChangeTick]=useState(0);
  const task=useMaintenanceTask(maintenance);
  // Other views can change tracks or playlists; open reports and reviews recheck themselves when shown.
  useEffect(()=>{
    let off:(()=>void)|undefined,gone=false;
    void maintenance.onChange(()=>setChangeTick(n=>n+1)).then(f=>{if(gone)f();else off=f;}).catch(()=>{});
    return()=>{gone=true;off?.();};
  },[maintenance]);
  // Only the visible tab reacts to a change, so two tabs never compete for the single operation slot.
  const tickFor=(id:Tab)=>tab===id?changeTick:0;
  return <section className="maintenance-view cleanup-view">
    <div className="view-title"><div><button className="back-link" onClick={onBack}><ChevronLeft size={15}/>Library</button><div className="eyebrow">LIBRARY TOOLS</div><h1>Maintenance</h1><p>Review duplicates, find missing files and reclaim space. Every change is previewed first.</p></div></div>
    <div className="cleanup-tabs" role="tablist" aria-label="Maintenance tools">{tabs.map(([id,name])=><button key={id} role="tab" aria-selected={tab===id} disabled={!!task.busy&&tab!==id} onClick={()=>setTab(id)}>{name}</button>)}</div>
    <div className="cleanup-body maint-body">
      <div hidden={tab!=="duplicates"}><DuplicatesTab library={bridge} bridge={maintenance} task={task} changeTick={tickFor("duplicates")} onLibraryChanged={onLibraryChanged}/></div>
      <div hidden={tab!=="broken"}><BrokenTab bridge={maintenance} task={task} changeTick={tickFor("broken")}/></div>
      <div hidden={tab!=="unused"}><UnusedTab library={bridge} bridge={maintenance} task={task} changeTick={tickFor("unused")} onLibraryChanged={onLibraryChanged}/></div>
      <div hidden={tab!=="backups"}><BackupsTab bridge={maintenance} task={task} changeTick={tickFor("backups")}/></div>
    </div>
  </section>;
}
