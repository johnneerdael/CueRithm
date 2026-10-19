import {Plus,Trash2,RotateCcw} from "lucide-react";
import {anchorRoles,cueKinds,customRow,cueTemplateError,emergencyRow,factoryTemplate,isLoop,sameTemplate,startBehaviors} from "../cue-template";
import type {CueMarker,CueTemplate,CueTemplateRow,StartBehavior} from "../cue-template";
import {cueColor,publishedColorNames} from "../display";

const structuralNames:Partial<Record<CueMarker,string>>={start:"Start",drop:"Drop",breakdown:"Breakdown",second_drop:"Second drop",second_breakdown:"Second breakdown",lastbeat:"End / fade-out",emergency_loop:"Emergency loop"};

/**
 * Edits the cue template that turns detected structure into saved cues. Structural rows publish at what the
 * detector found; custom rows sit a whole number of beats before or after a structural anchor.
 */
export default function CueTemplateEditor({template,emergencyEnabled,onChange}:{template:CueTemplate;emergencyEnabled:boolean;onChange:(template:CueTemplate)=>void}){
  // The emergency loop row is shown only while the detector setting is on; a missing row appears with its defaults.
  const hasEmergency=template.rows.some(r=>r.markerType==="emergency_loop");
  const shown:(CueTemplateRow&{index:number})[]=template.rows.map((row,index)=>({...row,index})).filter(r=>emergencyEnabled||r.markerType!=="emergency_loop");
  if(emergencyEnabled&&!hasEmergency)shown.push({...emergencyRow(),index:template.rows.length});
  const edit=(index:number,patch:Partial<CueTemplateRow>)=>{
    const rows=[...template.rows];
    if(index>=rows.length)rows.push(emergencyRow());
    rows[index]={...rows[index],...patch};
    onChange({...template,rows});
  };
  const error=cueTemplateError(template);
  const anchorName=(marker:CueMarker)=>template.customAnchors[marker]?.name??"";
  const setAnchor=(marker:CueMarker,name:string)=>{
    const next={...template.customAnchors};
    if(name)next[marker]={name};else delete next[marker];
    onChange({...template,customAnchors:next});
  };
  const colours=(row:CueTemplateRow)=>[...new Set([...publishedColorNames,...(row.color&&!publishedColorNames.includes(row.color)?[row.color]:[])])];
  return <div className="cue-template">
    <div className="cue-template-head"><div><strong>Cue template</strong><p>Choose which cues are saved, how they look, and where extra cues sit relative to the detected structure.</p></div>
      <button className="secondary" disabled={sameTemplate(template,factoryTemplate())} onClick={()=>onChange(factoryTemplate())}><RotateCcw size={14}/>Restore factory template</button></div>
    {error?<div role="alert" className="error-banner">{error}</div>:null}
    <table className="cue-template-table" aria-label="Cue template rows"><thead><tr><th>On</th><th>Cue</th><th>Name</th><th>Type</th><th>Colour</th><th>Placement</th><th/></tr></thead><tbody>
      {shown.map(row=>{const custom=row.markerType==="normal",label=custom?"Custom cue":structuralNames[row.markerType]??row.markerType;
        return <tr key={row.index} data-custom={custom||undefined}>
          <td><input type="checkbox" aria-label={`Include ${label} ${row.index+1}`} checked={row.checked} onChange={e=>edit(row.index,{checked:e.target.checked})}/></td>
          <td>{label}</td>
          <td><input aria-label={`Name of ${label} ${row.index+1}`} value={row.name} maxLength={128} onChange={e=>edit(row.index,{name:e.target.value})}/></td>
          <td><select aria-label={`Type of ${label} ${row.index+1}`} value={row.cueType} onChange={e=>{const cueType=e.target.value as CueTemplateRow["cueType"];edit(row.index,{cueType,loopDuration:isLoop(cueType)?row.loopDuration??(custom?8:16):row.loopDuration})}}>{cueKinds.map(k=><option key={k.id} value={k.id}>{k.label}</option>)}</select></td>
          <td><span className="cue-swatch" style={{background:cueColor(row.color)}} aria-hidden="true"/><select aria-label={`Colour of ${label} ${row.index+1}`} value={row.color??""} onChange={e=>edit(row.index,{color:e.target.value||null})}>{colours(row).map(c=><option key={c} value={c}>{c.replace(/_/g," ")}</option>)}</select></td>
          <td>{custom?<span className="cue-placement">
            <input type="number" min={0} step={1} aria-label={`Beats for custom cue ${row.index+1}`} value={row.beatsOffset??0} onChange={e=>edit(row.index,{beatsOffset:Number(e.target.value)})}/>beats
            <select aria-label={`Before or after for custom cue ${row.index+1}`} value={row.position??"before"} onChange={e=>edit(row.index,{position:e.target.value as "before"|"after"})}><option value="before">before</option><option value="after">after</option></select>
            <select aria-label={`Anchor for custom cue ${row.index+1}`} value={row.anchor??"drop"} onChange={e=>edit(row.index,{anchor:e.target.value as CueMarker})}>{anchorRoles.map(a=><option key={a.marker} value={a.marker}>{a.label}</option>)}</select>
            {isLoop(row.cueType)?<><input type="number" min={1} step={1} aria-label={`Loop length for custom cue ${row.index+1}`} value={row.loopDuration??8} onChange={e=>edit(row.index,{loopDuration:Number(e.target.value)})}/>beat loop</>:null}
          </span>:isLoop(row.cueType)?<span className="cue-placement"><input type="number" min={1} step={1} aria-label={`Loop length of ${label} ${row.index+1}`} value={row.loopDuration??16} onChange={e=>edit(row.index,{loopDuration:Number(e.target.value)})}/>beat loop</span>:<span className="cue-placement muted">At detected position</span>}</td>
          <td>{custom?<button className="icon-button" aria-label={`Remove custom cue ${row.index+1}`} onClick={()=>onChange({...template,rows:template.rows.filter((_,i)=>i!==row.index)})}><Trash2 size={14}/></button>:null}</td>
        </tr>;})}
    </tbody></table>
    <button className="secondary cue-add" disabled={template.rows.length>=64} onClick={()=>onChange({...template,rows:[...template.rows,customRow()]})}><Plus size={14}/>Add custom cue</button>
    <label className="setting-row"><div><strong>Start cue</strong><p>Where the Start cue sits. Cues measured from Start keep the detected position.</p></div>
      <select aria-label="Start cue placement" value={template.startCueBehavior??"atFirstBeat"} onChange={e=>onChange({...template,startCueBehavior:e.target.value as StartBehavior,useFirstCueAsStart:undefined})}>{startBehaviors.map(b=><option key={b.id} value={b.id}>{b.label}</option>)}</select></label>
    <label className="setting-row"><div><strong>Keep cue slots</strong><p>Save each cue in its template slot, leaving a gap when a section is not detected, so a hot cue button always means the same section.</p></div>
      <input type="checkbox" aria-label="Keep cue slots" checked={template.keepCuePositions} onChange={e=>onChange({...template,keepCuePositions:e.target.checked})}/></label>
    <label className="setting-row"><div><strong>Use my cues as anchors</strong><p>When a track already has a cue with a name below, that cue sets the section's position instead of the detector. Tracks without any of these cues, such as a first preparation, use the detected structure.</p></div>
      <input type="checkbox" aria-label="Use my cues as anchors" checked={template.enableCustomAnchors} onChange={e=>onChange({...template,enableCustomAnchors:e.target.checked})}/></label>
    {template.enableCustomAnchors?<fieldset className="cue-anchors"><legend>Anchor cue names</legend>
      {anchorRoles.filter(a=>emergencyEnabled||a.marker!=="emergency_loop").map(a=><label key={a.marker}>{a.label}<input aria-label={`${a.label} anchor cue name`} value={anchorName(a.marker)} maxLength={128} placeholder="Cue name" onChange={e=>setAnchor(a.marker,e.target.value)}/></label>)}
    </fieldset>:null}
  </div>;
}
