import {describe,it,expect} from "vitest";
import {cleanupValidationError} from "../cleanup-validation";
import {emptyCleanup} from "../views/CleanupSettings";
import type {CleanupConfig} from "../types";
import fixtures from "../../../../tools/fixtures/cleanup-contract.json";

const contract=fixtures as {schema:string;cases:{name:string;config:CleanupConfig;valid:boolean}[]};
describe("native cleanup configuration contract",()=>{
 it("uses the shared native contract version",()=>expect(contract.schema).toBe("cleanup-config-contract-v1"));
 it.each(contract.cases)("$name",({config,valid})=>expect(cleanupValidationError(config)===null).toBe(valid));
 it.each(["genres","artists","ignored","conversion"] as const)("counts unique $0 set values at the 4096 limit",owner=>{
  for(const count of [4096,4097]){
   const config=emptyCleanup(),values=Array.from({length:count},(_,i)=>`value-${i}`);
   if(owner==="ignored")config.fixes=[{id:"rule",enabled:false,ignored_values:values,rule:{kind:"decode_entities"}}];
   else if(owner==="conversion"){config.genres.enabled=true;config.genres.conversion={category:"Category",genres:values,remove_genre:false};}
   else config[owner].protected_values=values;
   expect(cleanupValidationError(config)===null).toBe(count<=4096);
  }
 });
 it.each(["genres","artists"] as const)("bounds enabled $0 mappings and preserves disabled drafts",owner=>{
  const config=emptyCleanup();config[owner].mappings=Array.from({length:4097},(_,i)=>({from:`source-${i}`,to:null}));
  expect(cleanupValidationError(config)).toBeNull();config[owner].enabled=true;
  expect(cleanupValidationError(config)).not.toBeNull();config[owner].mappings.pop();expect(cleanupValidationError(config)).toBeNull();
 });
 it("rejects nonfinite numeric drafts and malformed UTF16 before native deserialization",()=>{
  for(const part of [NaN,Infinity,-Infinity]){
   const config=emptyCleanup();config.fixes=[{id:"rule",enabled:false,ignored_values:[],rule:{kind:"extract_artist",separator:" - ",part}}];expect(cleanupValidationError(config)).not.toBeNull();
  }
  for(const field of ["id","ignored","disabled text","mapping","conversion"]){
   const config=emptyCleanup();
   if(field==="mapping")config.genres.mappings=[{from:"\ud800",to:null}];
   else if(field==="conversion")config.genres.conversion={category:"\ud800",genres:[],remove_genre:false};
   else config.fixes=[{id:field==="id"?"\ud800":"rule",enabled:false,ignored_values:field==="ignored"?["\ud800"]:[],rule:{kind:"replace_characters",characters:field==="disabled text"?"\ud800":"_"}}];
   expect(cleanupValidationError(config)).not.toBeNull();
  }
 });
});
