import {describe,it,expect,vi} from "vitest";
import {render,screen} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {useState} from "react";
import {ImportPolicy} from "../views/ImportPolicy";
import {defaultImportPolicy,processingRequested,type ImportPolicy as Policy} from "../import-types";
import {importProfile} from "./import-fixtures";
describe("import processing choices",()=>{
 it("store_as_imported_starts_with_all_processing_off_even_when_incoming_is_on",()=>{
  const incoming={analysis:true,tagging:true,rename:true,replace_hotcues:true};expect(Object.values(incoming).every(Boolean)).toBe(true);
  const value=defaultImportPolicy();render(<ImportPolicy value={value} onChange={()=>{}} profile={importProfile}/>);
  for(const name of ["Analyse key","Analyse energy","Generate cues","Replace existing hotcues","Retag / enrich metadata","Apply cleanup rules","Write audio tags","Rename files"]){expect(screen.getByRole("checkbox",{name})).not.toBeChecked();}
  expect(processingRequested(value)).toBe(false);expect(value.include_history).toBe(false);expect(value.include_queue).toBe(false);
 });
 it.each([
  ["Analyse key","analysis_fields",{key:"fill_missing"}],
  ["Analyse energy","analysis_fields",{energy:"fill_missing"}],
  ["Generate cues","cue_generation","generate"],
  ["Replace existing hotcues","replace_hotcues",true],
  ["Retag / enrich metadata","metadata_enrichment",expect.objectContaining({enabled:true})],
  ["Apply cleanup rules","cleanup",expect.objectContaining({enabled:true})],
  ["Write audio tags","write_audio_tags",expect.arrayContaining(["title"])],
  ["Rename files","relocation",expect.objectContaining({mode:"rename_in_place"})],
 ] as const)("independently toggles %s",async(name,key,expected)=>{
  const value=defaultImportPolicy(),changed=vi.fn();render(<ImportPolicy value={value} onChange={changed} profile={importProfile}/>);
  await userEvent.click(screen.getByRole("checkbox",{name}));const next=changed.mock.lastCall![0] as Policy;
  expect(next[key]).toEqual(expected);const rest={...next};delete (rest as Partial<Policy>)[key];const original={...value};delete (original as Partial<Policy>)[key];expect(rest).toEqual(original);
 });
 it("overwrite_alone_generates_nothing",()=>{const policy={...defaultImportPolicy(),replace_hotcues:true};expect(processingRequested(policy)).toBe(false);});
 it("rename_defaults_to_in_place",async()=>{const changed=vi.fn();render(<ImportPolicy value={defaultImportPolicy()} onChange={changed} profile={importProfile}/>);await userEvent.click(screen.getByRole("checkbox",{name:"Rename files"}));expect(changed.mock.lastCall![0].relocation).toEqual({mode:"rename_in_place",rule:"{artist} - {title}"});});
 it("capability_losses_are_visible",()=>{const profile=structuredClone(importProfile);profile.fields.beatgrid={read:false,write:false,lossless:false,evidence:"d0",reason:"Source grid profile unavailable"};render(<ImportPolicy value={defaultImportPolicy()} onChange={()=>{}} profile={profile}/>);expect(screen.getByRole("checkbox",{name:"Import beatgrid"})).toBeDisabled();expect(screen.getByText("Source grid profile unavailable")).toBeVisible();});
 it("fields_gate_new_and_existing_publication",async()=>{function Example(){const [value,setValue]=useState(defaultImportPolicy());return <><ImportPolicy value={value} onChange={setValue} profile={importProfile}/><output>{JSON.stringify(value.selected_fields)}</output></>;}render(<Example/>);await userEvent.click(screen.getByRole("checkbox",{name:"Import key"}));expect(screen.getByRole("status").textContent).not.toContain('"key"');expect(screen.getByRole("status").textContent).toContain('"energy"');});
});
