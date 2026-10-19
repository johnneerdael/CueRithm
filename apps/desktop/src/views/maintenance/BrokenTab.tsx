import {useEffect,useMemo,useState} from "react";
import {ScanSearch} from "lucide-react";
import type {MaintenanceBridge} from "../../maintenance-api";
import type {BrokenScanReport,MaintenanceFileState} from "../../maintenance-types";
import type {MaintenanceTask} from "../../use-maintenance-task";
import {StaleNotice,TaskStatus,ShowMore,usePaged} from "./shared";

export const stateLabels:Record<MaintenanceFileState,string>={
  present:"Present",missing:"Missing",unavailable:"Unavailable",symlink:"Symbolic link",not_regular:"Not a regular file",changed:"Changed since scan",
  video:"Video without audio",no_audio:"No audio stream",decode_error:"Decode error",unsupported:"Unsupported format",timed_out:"Timed out",duration_skipped:"Skipped (duration limit)",
};
const stateHelp:Partial<Record<MaintenanceFileState,string>>={
  missing:"The file is no longer at its recorded path.",unavailable:"The location could not be read. A disconnected drive looks like this; reconnect it before treating files as lost.",
  timed_out:"Reading took too long. Try again when the drive is idle.",decode_error:"The file exists but could not be decoded.",
};

export default function BrokenTab({bridge,task,changeTick}:{bridge:MaintenanceBridge;task:MaintenanceTask;changeTick:number}){
  const [inspect,setInspect]=useState(false),[report,setReport]=useState<BrokenScanReport|null>(null),[filter,setFilter]=useState<"problems"|"all">("problems");
  const scan=()=>void task.run(inspect?"Decoding library files…":"Checking library files…","broken",async id=>{setReport(await bridge.scanBroken(id,{inspect_audio:inspect}));});
  // Another view changed the library: recheck the saved report's source proofs so staleness is shown.
  const reportId=report?.id;
  useEffect(()=>{if(!reportId||!changeTick)return;let live=true;void task.run("Rechecking report…","broken",async id=>{const next=await bridge.brokenScan(id,reportId);if(live)setReport(next);});return()=>{live=false;};
  // eslint-disable-next-line react-hooks/exhaustive-deps
  },[changeTick]);
  const rows=useMemo(()=>(report?.tracks??[]).filter(t=>filter==="all"||t.state!=="present"||t.source_changed),[report,filter]);
  const counts=useMemo(()=>{const map=new Map<MaintenanceFileState,number>();for(const t of report?.tracks??[])map.set(t.state,(map.get(t.state)??0)+1);return map;},[report]);
  const paged=usePaged(rows);
  const problems=(report?.tracks.length??0)-(counts.get("present")??0);
  return <section className="maint-panel" aria-label="Missing and broken files">
    <p className="cleanup-note">Find library tracks whose files are missing, unreadable or cannot be decoded. Nothing is changed: this screen only reports. Archived and active tracks are both checked.</p>
    <div className="maint-controls">
      <label><input type="checkbox" checked={inspect} disabled={!!task.busy} onChange={e=>setInspect(e.target.checked)}/>Decode each file (slower, finds corrupt audio)</label>
      <button className="primary" disabled={!!task.busy} onClick={scan}><ScanSearch size={14}/>Scan library</button>
    </div>
    <TaskStatus task={task}/>
    {report?<>
      <StaleNotice stale={report.stale} reason={report.stale_reason} what="report"/>
      <div className="maint-summary" role="status">
        <strong>{report.tracks.length.toLocaleString()} tracks checked · {problems.toLocaleString()} {problems===1?"problem":"problems"}</strong>
        <span>{[...counts].filter(([s])=>s!=="present").map(([s,n])=>`${stateLabels[s]} ${n.toLocaleString()}`).join(" · ")||"Every file was found."}</span>
        {report.decoder_identity?<small>Decoder {report.decoder_identity}</small>:<small>Presence only: a present file may still fail to decode.</small>}
      </div>
      <div className="maint-filter" role="group" aria-label="Show">
        <button aria-pressed={filter==="problems"} onClick={()=>setFilter("problems")}>Problems</button><button aria-pressed={filter==="all"} onClick={()=>setFilter("all")}>All tracks</button>
      </div>
      {rows.length?<table className="maint-table"><thead><tr><th>File</th><th>State</th><th>Detail</th></tr></thead><tbody>
        {paged.visible.map(t=><tr key={t.id}><td title={t.display_path}>{t.display_path}</td>
          <td><span className={"maint-state "+(t.state==="present"?"ok":"bad")}>{stateLabels[t.state]}</span>{t.source_changed?<span className="maint-state warn">File changed</span>:null}</td>
          <td>{t.detail??stateHelp[t.state]??""}</td></tr>)}
      </tbody></table>:<p className="cleanup-note">No problems found.</p>}
      <ShowMore {...paged}/>
    </>:null}
  </section>;
}
