import factory from "./factory-cue-template.json";

export type CueMarker="start"|"drop"|"breakdown"|"second_drop"|"second_breakdown"|"lastbeat"|"emergency_loop"|"normal";
export type CueKind="hotcue"|"loop_hot"|"loop_active"|"memory_cue"|"memory_cue_loop";
export type StartBehavior="atFirstBeat"|"atExistingCue"|"atZero";
export interface CueTemplateRow {checked:boolean;markerType:CueMarker;name:string;cueType:CueKind;color:string|null;label?:string;anchor?:CueMarker;position?:"before"|"after";beatsOffset?:number;loopDuration?:number}
export interface CueTemplate {overwrite:boolean;startCueBehavior?:StartBehavior;useFirstCueAsStart?:boolean;breakdownMinBeats:number;keepCuePositions:boolean;enableCustomAnchors:boolean;dropAtStart:string;customAnchors:Record<string,{name:string}>;rows:CueTemplateRow[]}
export interface TempoRange {min_tempo:number;max_tempo:number}

export const anchorRoles:{marker:CueMarker;label:string}[]=[
  {marker:"start",label:"Start"},{marker:"drop",label:"Drop"},{marker:"breakdown",label:"Breakdown"},{marker:"second_drop",label:"Second drop"},
  {marker:"second_breakdown",label:"Second breakdown"},{marker:"lastbeat",label:"End / fade-out"},{marker:"emergency_loop",label:"Emergency loop"},
];
export const cueKinds:{id:CueKind;label:string}[]=[
  {id:"hotcue",label:"Hot cue"},{id:"loop_hot",label:"Hot loop"},{id:"loop_active",label:"Active loop"},{id:"memory_cue",label:"Memory cue"},{id:"memory_cue_loop",label:"Memory loop"},
];
export const isLoop=(kind:CueKind)=>kind==="loop_hot"||kind==="loop_active"||kind==="memory_cue_loop";
export const startBehaviors:{id:StartBehavior;label:string}[]=[
  {id:"atFirstBeat",label:"At the first beat"},{id:"atExistingCue",label:"At the track's first existing cue"},{id:"atZero",label:"At the start of the track"},
];

// Same presets the reference analysis dialog offers; "Any" is clamped to the supported 50–300 span.
export const tempoPresets:{id:string;label:string;range:TempoRange}[]=[
  {id:"50_300",label:"Any",range:{min_tempo:50,max_tempo:300}},{id:"58_115",label:"58 – 115",range:{min_tempo:58,max_tempo:115}},{id:"68_135",label:"68 – 135",range:{min_tempo:68,max_tempo:135}},
  {id:"78_155",label:"78 – 155",range:{min_tempo:78,max_tempo:155}},{id:"88_175",label:"88 – 175",range:{min_tempo:88,max_tempo:175}},{id:"98_195",label:"98 – 195 (default)",range:{min_tempo:98,max_tempo:195}},
  {id:"128_255",label:"128 – 255",range:{min_tempo:128,max_tempo:255}},{id:"180_300",label:"180+",range:{min_tempo:180,max_tempo:300}},
];
export const defaultTempoId="98_195";
export function tempoId(range:TempoRange|null|undefined){
  if(!range)return defaultTempoId;
  return tempoPresets.find(p=>p.range.min_tempo===range.min_tempo&&p.range.max_tempo===range.max_tempo)?.id??"custom";
}

export const factoryTemplate=():CueTemplate=>JSON.parse(JSON.stringify(factory)) as CueTemplate;
export const emergencyRow=():CueTemplateRow=>({checked:true,markerType:"emergency_loop",name:"S.O.S.",label:"Emergency Loop",cueType:"loop_active",color:"magenta",loopDuration:16});
export const customRow=():CueTemplateRow=>({checked:true,markerType:"normal",name:"Custom cue",cueType:"hotcue",color:"green",anchor:"drop",position:"before",beatsOffset:16});
export const sameTemplate=(a:CueTemplate|null|undefined,b:CueTemplate)=>JSON.stringify(a??factoryTemplate())===JSON.stringify(b);

const sameSlot=(a:CueTemplateRow,b:CueTemplateRow)=>a.cueType===b.cueType&&a.markerType===b.markerType&&a.anchor===b.anchor&&a.position===b.position&&a.beatsOffset===b.beatsOffset;
/** Mirrors the engine's user-template validation so the editor can explain a problem before saving. */
export function cueTemplateError(template:CueTemplate):string|null{
  if(template.rows.length>64)return "A cue template can have at most 64 rows.";
  for(const [index,row] of template.rows.entries()){
    const n=index+1;
    if(!row.name.trim())return `Cue row ${n} needs a name.`;
    if(row.name.length>128)return `Cue row ${n} has a name that is too long.`;
    if(row.markerType==="normal"){
      if(!row.anchor||row.anchor==="normal")return `Cue row ${n} needs an anchor.`;
      if(!row.position)return `Cue row ${n} needs before or after.`;
      if(row.beatsOffset===undefined||!Number.isInteger(row.beatsOffset)||row.beatsOffset<0||row.beatsOffset>1e9)return `Cue row ${n} needs a whole number of beats, zero or more.`;
      if(isLoop(row.cueType)&&!(row.loopDuration!==undefined&&row.loopDuration>0))return `Cue row ${n} is a loop and needs a length in beats.`;
    }
    for(const other of template.rows.slice(0,index)){
      if(row.checked&&other.checked&&sameSlot(row,other))return `Cue rows ${template.rows.indexOf(other)+1} and ${n} would create the same cue.`;
    }
  }
  if(template.enableCustomAnchors){
    for(const [role,anchor] of Object.entries(template.customAnchors)){
      if(!anchor.name.trim()||anchor.name.length>128)return `The ${role.replace(/_/g," ")} anchor needs a cue name.`;
    }
  }
  return null;
}
