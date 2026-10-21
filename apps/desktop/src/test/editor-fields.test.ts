import {describe,expect,it} from "vitest";
import {dateText,parseMetadataDate,setTypedInput} from "../editor-fields";
import {emptyMetadataPatch} from "../editor-types";
describe("typed metadata form values",()=>{
  it("preserves year and month precision without inventing dates",()=>{
    expect(parseMetadataDate("2026")).toEqual({year:2026,month:null,day:null});
    expect(parseMetadataDate("2026-10")).toEqual({year:2026,month:10,day:null});
    expect(dateText({year:2026,month:10,day:null})).toBe("2026-10");
    for(const value of ["2026-","2026-02-29","2026-13","0000","2026-1"]){expect(()=>parseMetadataDate(value)).toThrow();}
  });
  it("persists incomplete input separately and never leaves an older valid value publishable",()=>{
    let patch=setTypedInput(emptyMetadataPatch(),"release_date","set","2026-10");
    patch=setTypedInput(patch,"release_date","set","2026-");
    expect(patch.form_values).toEqual({release_date:"2026-"});
    expect(patch.fields.release_date).toEqual({kind:"keep"});
    patch=setTypedInput(patch,"release_date","set","2026-11");
    expect(patch.form_values).toEqual({});
    expect(patch.fields.release_date).toEqual({kind:"set",value:{year:2026,month:11,day:null}});
  });
  it("retains invalid numeric input across a native draft roundtrip and clears it explicitly",()=>{
    const patch=setTypedInput(emptyMetadataPatch(),"track_total","set","999999");
    expect(JSON.parse(JSON.stringify(patch)).form_values.track_total).toBe("999999");
    expect(setTypedInput(patch,"track_total","clear","").form_values).toEqual({});
    expect(setTypedInput(patch,"track_total","clear","").fields.track_total).toEqual({kind:"clear"});
    expect(setTypedInput(emptyMetadataPatch(),"track_number","set","0").form_values).toEqual({track_number:"0"});
    expect(setTypedInput(emptyMetadataPatch(),"bpm","set","1000").form_values).toEqual({bpm:"1000"});
  });
});
