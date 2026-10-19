import type {CanonicalMapping,CleanupConfig,SmartFix} from "./types";

// Match Rust char::is_whitespace; JavaScript trim differs for NEXT LINE and BOM.
export const rustTrim=(value:string)=>value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu,"");
const validUnicode=(value:string)=>!Array.from(value).some(character=>{const code=character.codePointAt(0)!;return code>=0xd800&&code<=0xdfff;});
const utf8Bytes=(value:string)=>new TextEncoder().encode(value).length;
const unicodeError=(value:string,label:string)=>validUnicode(value)?null:`${label} must contain valid Unicode characters.`;
export const validArtistPart=(part:number,enabled:boolean)=>Number.isInteger(part)&&part>=(enabled?1:0)&&part<=(enabled?256:65535);
export const artistPartMessage="Result number must be a finite whole number from 1 to 256.";
export function smartFixTextError(rule:SmartFix):string|null {
 let value:string,label:string;
 switch(rule.kind){
  case "replace_characters":value=rule.characters;label="Characters";break;
  case "extract_artist":value=rule.separator;label="Separator";break;
  case "extract_remixer":value=rule.word;label="Remixer suffix";break;
  default:return null;
 }
 const invalidUnicode=unicodeError(value,label);if(invalidUnicode)return invalidUnicode;
 if(Array.from(value).length>64)return `${label} must contain at most 64 characters.`;
 if(rule.kind==="extract_artist"&&!value.length)return "Separator must contain 1 to 64 characters.";
 if(rule.kind==="extract_remixer"&&!rustTrim(value))return "Remixer suffix must contain 1 to 64 characters, including a non-whitespace character.";
 return null;
}
function protectedError(values:string[],label:string):string|null {
 const unique=new Set(values);
 if(unique.size>4096)return `${label}: use at most 4096 unique values.`;
 for(const value of unique){
  const invalidUnicode=unicodeError(value,label);if(invalidUnicode)return invalidUnicode;
  if(!rustTrim(value)||utf8Bytes(value)>4096)return `${label}: each value must be nonblank and at most 4096 UTF-8 bytes.`;
 }
 return null;
}
function mappingError(mappings:CanonicalMapping[],label:string,enabled:boolean):string|null {
 if(enabled&&mappings.length>4096)return `${label}: use at most 4096 mappings.`;
 const table=new Map<string,string|null>();
 for(const [index,mapping] of mappings.entries()){
  const prefix=`${label} mapping ${index+1}`;
  const sourceUnicode=unicodeError(mapping.from,`${prefix} source`);if(sourceUnicode)return sourceUnicode;
  if(mapping.to!==null){const targetUnicode=unicodeError(mapping.to,`${prefix} target`);if(targetUnicode)return targetUnicode;}
  if(!enabled)continue;
  if(!mapping.from.length)return `${prefix}: enter a source value.`;
  if(utf8Bytes(mapping.from)>4096)return `${prefix}: source must be at most 4096 UTF-8 bytes.`;
  if(table.has(mapping.from))return `${prefix}: source duplicates an earlier mapping.`;
  if(mapping.to!==null&&(!rustTrim(mapping.to)||utf8Bytes(mapping.to)>4096))return `${prefix}: enter a nonblank canonical value of at most 4096 UTF-8 bytes, or select Remove.`;
  table.set(mapping.from,mapping.to===null?null:rustTrim(mapping.to));
 }
 const complete=new Set<string>();
 for(const start of table.keys()){
  const seen=new Set<string>();let current=start;
  while(!complete.has(current)){
   const next=table.get(current);if(next==null||next===current)break;
   if(seen.has(current))return `${label} mappings contain a cycle. Change a target to end the chain.`;
   seen.add(current);current=next;
  }
  for(const source of seen)complete.add(source);
 }
 return null;
}
export function cleanupValidationError(config?:CleanupConfig|null):string|null {
 if(!config)return null;
 if(config.fixes.length>64)return "Use at most 64 Smart Fixes.";
 const ids=new Set<string>();
 for(const [index,fix] of config.fixes.entries()){
  const label=`Smart Fix ${index+1}`;
  const idUnicode=unicodeError(fix.id,`${label} ID`);if(idUnicode)return idUnicode;
  if(!rustTrim(fix.id)||utf8Bytes(fix.id)>128||ids.has(fix.id))return `${label}: rule IDs must be nonblank, unique and at most 128 UTF-8 bytes.`;
  ids.add(fix.id);
  const ignored=protectedError(fix.ignored_values,`${label} ignored values`);if(ignored)return ignored;
  const rule=fix.rule;
  // Disabled drafts still have to deserialize as native strings and u16 values.
  if(rule.kind==="extract_artist"&&!validArtistPart(rule.part,fix.enabled))return artistPartMessage;
  const text=rule.kind==="replace_characters"?rule.characters:rule.kind==="extract_artist"?rule.separator:rule.kind==="extract_remixer"?rule.word:null;
  if(text!==null){const invalidUnicode=unicodeError(text,label);if(invalidUnicode)return invalidUnicode;}
  const error=fix.enabled?smartFixTextError(rule):null;if(error)return error;
 }
 for(const kind of ["genres","artists"] as const){
  const error=protectedError(config[kind].protected_values,kind==="genres"?"Genre protected values":"Artist protected values");if(error)return error;
 }
 const genres=mappingError(config.genres.mappings,"Genre cleanup",config.genres.enabled);if(genres)return genres;
 const conversion=config.genres.conversion;
 if(conversion){
  const categoryUnicode=unicodeError(conversion.category,"Genre tag category");if(categoryUnicode)return categoryUnicode;
  for(const genre of conversion.genres){const error=unicodeError(genre,"Converted genre");if(error)return error;}
  if(config.genres.enabled){
   if(!rustTrim(conversion.category)||utf8Bytes(conversion.category)>256)return "Genre tag category must be nonblank and at most 256 UTF-8 bytes.";
   if(new Set(conversion.genres).size>4096)return "Select at most 4096 unique genres to convert.";
  }
 }
 if(config.artists.enabled&&!config.artists.roles.length)return "Artist cleanup: select at least one contributor role.";
 return mappingError(config.artists.mappings,"Artist cleanup",config.artists.enabled);
}
