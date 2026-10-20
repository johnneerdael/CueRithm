import type {MetadataDate,MetadataEditorPatch,MetadataTextField,MetadataValues,ValueEdit} from "./editor-types";
export type TypedField="release_date"|"original_date"|"track_number"|"track_total"|"disc_number"|"disc_total"|"bpm";
export const textFields:{field:MetadataTextField;label:string;multiple?:boolean;group:"recording"|"release"|"contributors"|"overrides"}[]=[
  {field:"title",label:"Title",group:"recording"},{field:"artist",label:"Artists",multiple:true,group:"recording"},{field:"version",label:"Version / mix",group:"recording"},
  {field:"genre",label:"Genres",multiple:true,group:"recording"},{field:"style",label:"Styles",multiple:true,group:"recording"},
  {field:"album",label:"Album",group:"release"},{field:"album_artist",label:"Album artists",multiple:true,group:"release"},{field:"label",label:"Label",group:"release"},
  {field:"catalog_number",label:"Catalog number",group:"release"},{field:"isrc",label:"ISRC",group:"release"},
  {field:"remixer",label:"Remixers",multiple:true,group:"contributors"},{field:"producer",label:"Producers",multiple:true,group:"contributors"},{field:"composer",label:"Composers",multiple:true,group:"contributors"},{field:"lyricist",label:"Lyricists",multiple:true,group:"contributors"},{field:"grouping",label:"Grouping",multiple:true,group:"contributors"},
  {field:"key",label:"File key tag",group:"overrides"},
];
export function dateText(value:MetadataDate|null){return value?String(value.year).padStart(4,"0")+(value.month!==null?"-"+String(value.month).padStart(2,"0"):"")+(value.day!==null?"-"+String(value.day).padStart(2,"0"):""):"";}
export function parseMetadataDate(raw:string):MetadataDate{
  if(!/^\d{4}(?:-\d{2}(?:-\d{2})?)?$/.test(raw))throw Error("Use YYYY, YYYY-MM or YYYY-MM-DD.");
  const [year,month,day]=raw.split("-").map(Number);
  if(year<1||year>9999||month!==undefined&&(month<1||month>12))throw Error("Enter a valid date.");
  const leap=year%4===0&&(year%100!==0||year%400===0),days=[31,leap?29:28,31,30,31,30,31,31,30,31,30,31];
  if(day!==undefined&&(day<1||day>days[month-1]))throw Error("Enter a valid day for this month.");
  return {year,month:month??null,day:day??null};
}
export function setTypedInput(patch:MetadataEditorPatch,field:TypedField,kind:"keep"|"set"|"clear",raw:string):MetadataEditorPatch{
  const form_values={...patch.form_values};delete form_values[field];
  let value:ValueEdit<MetadataDate>|ValueEdit<number>={kind:kind==="clear"?"clear":"keep"};
  if(kind==="set"){
    try{
      if(field==="release_date"||field==="original_date")value={kind:"set",value:parseMetadataDate(raw)};
      else {
        const number=Number(raw);
        if(!raw.trim()||!Number.isFinite(number)||number<=0||field==="bpm"&&number>=1000||field!=="bpm"&&(!/^\d+$/.test(raw)||!Number.isInteger(number)||number>65535))throw Error("Enter a valid positive number.");
        value={kind:"set",value:number};
      }
    }catch{form_values[field]=raw;}
  }
  return {...patch,form_values,fields:{...patch.fields,[field]:value}};
}
export function mixedValue<T>(values:T[]):{mixed:boolean;value:T|undefined}{const value=values[0];return {mixed:values.some(other=>JSON.stringify(other)!==JSON.stringify(value)),value};}
export function snapshotValue(snapshot:MetadataValues,field:TypedField){const value=snapshot[field];return field==="release_date"||field==="original_date"?dateText(value as MetadataDate|null):value===null?"":String(value);}
export function typedInput(patch:MetadataEditorPatch,field:TypedField,fallback:string){
  if(Object.prototype.hasOwnProperty.call(patch.form_values,field))return patch.form_values[field];
  const value=patch.fields[field];
  return value.kind==="set"?(field==="release_date"||field==="original_date"?dateText(value.value as MetadataDate):String(value.value)):value.kind==="clear"?"":fallback;
}
