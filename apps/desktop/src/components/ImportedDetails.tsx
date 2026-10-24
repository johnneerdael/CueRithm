import {useEffect,useState} from "react";
import InspectorFrame from "./InspectorFrame";
import {importApi,type ImportBridge} from "../import-api";
import type {ImportedItemDetail} from "../import-types";
import {displayImportedValue} from "../views/ImportSelection";
export default function ImportedDetails({id,onClose,api=importApi}:{id:string|null;onClose:()=>Promise<void>|void;api?:ImportBridge}){
 const [detail,setDetail]=useState<ImportedItemDetail|null>(null),[error,setError]=useState<string|null>(null);
 useEffect(()=>{let cancelled=false;setDetail(null);setError(null);if(id)void api.importedItem(id).then(value=>{if(!cancelled)setDetail(value);}).catch(error=>{if(!cancelled)setError(error&&typeof error==="object"&&"message" in error?String(error.message):String(error));});return()=>{cancelled=true;};},[id,api]);
 return <InspectorFrame title="Imported track" description="Accepted values and separate source observations" onClose={onClose}>
  {error?<p role="alert">{error}</p>:!id?<p>Select one imported track.</p>:!detail?<p role="status">Loading imported details…</p>:<>
   <p>{detail.analysis_state} · {detail.availability}</p><p>{detail.playback_allowed?"Media is available for playback.":"Resolve the media location before playback."}</p>
   <h3>Accepted library values</h3><dl>{Object.entries(detail.active_fields).map(([field,value])=><div key={field}><dt>{field.replace(/^metadata\./,"").replaceAll("_"," ")}</dt><dd>{displayImportedValue(value)}<details><summary>Value origin</summary><pre>{JSON.stringify(detail.provenance[field]??null,null,2).slice(0,4096)}</pre></details></dd></div>)}</dl>
   <h3>Source observations</h3><p>Raw audio tags and cached source tags remain separate from accepted library values. Native timing needs a matching alignment profile before performance data can be exported.</p>
   {detail.observations.map(record=><details key={`${record.source_id}-${record.external_id}`}><summary>{record.external_id} · {record.track.locations.original??record.track.locations.availability}</summary><dl><dt>Source key</dt><dd>{record.track.key?.camelot??record.track.key?.raw??"Not supplied"}</dd><dt>Source metadata</dt><dd><pre>{JSON.stringify(record.track.metadata,null,2).slice(0,4096)}</pre></dd><dt>Cached source tags</dt><dd><pre>{JSON.stringify(record.track.cached_tags,null,2).slice(0,4096)}</pre></dd><dt>Raw audio tags</dt><dd>{record.track.file_tags?"Captured separately":"Not captured"}</dd><dt>Native grids and cues</dt><dd><pre>{JSON.stringify({grid:record.track.grid,cues:record.track.cues},null,2).slice(0,8192)}</pre></dd></dl>{record.track.issues.map((issue,index)=><p key={index}>{issue.message}</p>)}</details>)}
  </>}
 </InspectorFrame>;
}
