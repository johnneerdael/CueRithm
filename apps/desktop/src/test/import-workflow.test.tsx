import {describe,it,expect,vi} from "vitest";
import {render,screen,waitFor} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {LibraryImport} from "../views/LibraryImport";
import {importFixtureApi,importTrack} from "./import-fixtures";
import type {ImportEffectsPage} from "../import-types";
describe("import workflow",()=>{
 it("shows actual file changes and the complete cue family before applying effects",async()=>{
  const tail="Last generated cue after the previous display limit";
  const summary={processed:1,reused:0,unchanged:0,blocked:0,failed:0,cancelled:0,recovery_required:0};
  const api=importFixtureApi({
   applyImport:vi.fn(async()=>({operation_id:"durable-operation",tracks_created:1,fields_changed:3,collections_changed:1,processing_requested:true})),
   runImportProcessing:vi.fn(async()=>summary),
   previewImportEffects:vi.fn(async()=>({preview_id:"actual-effects",operation_id:"durable-operation",ready:3,blocked:0,unchanged:0})),
   importEffectsPage:vi.fn(async():Promise<ImportEffectsPage>=>({records:[
    {item_id:"item-one",stage:"file.tags",outcome:"ready",reason:null,changes:{audio_tags:[{before:{text:{title:["Old title"]}},after:{text:{title:["Reviewed title"]}}}]}},
    {item_id:"item-one",stage:"file.relocation",outcome:"ready",reason:null,changes:{files:[{before_path:"/example/original.wav",after_path:"/example/copies/Reviewed title.wav"}]}},
    {item_id:"item-one",stage:"generation.cues",outcome:"ready",reason:null,changes:{fields:{"cues.hot":{cues:[{name:"x".repeat(4200)},{name:tail}]}}}},
   ],next_cursor:null})),
   applyImportEffects:vi.fn(async()=>summary),
  });
  render(<LibraryImport api={api} onClose={()=>{}}/>);
  await userEvent.click(screen.getByRole("button",{name:"Choose library"}));
  await userEvent.click(await screen.findByRole("button",{name:"Inspect library"}));
  await userEvent.click(await screen.findByRole("button",{name:"Processing choices"}));
  await userEvent.click(screen.getByRole("button",{name:"Review import"}));
  await userEvent.click(await screen.findByRole("button",{name:"Apply import"}));
  await userEvent.click(await screen.findByRole("button",{name:"Generate selected processing"}));
  await userEvent.click(await screen.findByRole("button",{name:"Review processing effects"}));
  expect(await screen.findByText(/Old title/)).toBeVisible();
  expect(screen.getByText(/Reviewed title.wav/)).toBeVisible();
  expect(screen.getByText(new RegExp(tail))).toBeVisible();
  await userEvent.click(screen.getByRole("button",{name:"Apply processing effects"}));
  expect(api.applyImportEffects).toHaveBeenCalledWith(expect.any(String),"actual-effects");
 });
 it("reviews server-held effects and applies only their preview ID",async()=>{
  const api=importFixtureApi();render(<LibraryImport api={api} onClose={()=>{}}/>);
  await userEvent.selectOptions(screen.getByLabelText("Source app"),"mixxx");
  await userEvent.click(screen.getByRole("button",{name:"Choose library"}));
  await userEvent.click(await screen.findByRole("button",{name:"Inspect library"}));
  expect(await screen.findByText("Imported title")).toBeVisible();
  expect(screen.getByText("invented-mixxx-schema40")).toBeVisible();
  expect(screen.getByText(/^Missing media \//i)).toBeVisible();
  expect(screen.getByText("8A")).toBeVisible();
  await userEvent.click(screen.getByRole("button",{name:"Processing choices"}));
  await userEvent.click(screen.getByRole("button",{name:"Review import"}));
  await waitFor(()=>expect(api.previewImport).toHaveBeenCalledOnce());
  expect(vi.mocked(api.previewImport).mock.lastCall![1].policy.analysis_fields).toEqual({});
  expect(vi.mocked(api.previewImport).mock.lastCall![1].policy.relocation).toEqual({mode:"off"});
  await userEvent.click(await screen.findByRole("button",{name:"Apply import"}));
  await waitFor(()=>expect(api.applyImport).toHaveBeenCalledWith(expect.any(String),"server-held-preview"));
  expect(await screen.findByText(/1 track imported/i)).toBeVisible();
 });
 it("sends only the chosen observed candidate ID for preview",async()=>{
  const track=structuredClone(importTrack);
  track.extensions.field_candidates={title:[{id:"sql:title",surface:"sql",value:"SQL title",normalized_field:"metadata.title",normalized_value:{presence:"value",value:"SQL title"}},{id:"json:title",surface:"json",value:"JSON title",normalized_field:"metadata.title",normalized_value:{presence:"value",value:"JSON title"}}]};
  const api=importFixtureApi({importPage:vi.fn(async()=>({records:[{record_type:"track" as const,record:track}],total:1,next_cursor:null}))});
  render(<LibraryImport api={api} onClose={()=>{}}/>);
  await userEvent.click(screen.getByRole("button",{name:"Choose library"}));
  await userEvent.click(await screen.findByRole("button",{name:"Inspect library"}));
  await userEvent.selectOptions(await screen.findByLabelText("Resolve source title"),"json:title");
  await userEvent.click(screen.getByRole("button",{name:"Processing choices"}));
  await userEvent.click(screen.getByRole("button",{name:"Review import"}));
  await waitFor(()=>expect(api.previewImport).toHaveBeenCalledOnce());
  expect(vi.mocked(api.previewImport).mock.lastCall![1].candidate_choices).toEqual([{external_id:track.external_id,field:"title",candidate_id:"json:title"}]);
 });
 it("keeps history and queue off and requires an explicit native path prefix",async()=>{
  const api=importFixtureApi({chooseImportMapping:vi.fn(async()=>({root_id:"local-mapping",display_location:"/example/music"}))});
  render(<LibraryImport api={api} onClose={()=>{}}/>);
  expect(screen.getByLabelText("Include source history")).not.toBeChecked();
  expect(screen.getByLabelText("Include source queue")).not.toBeChecked();
  expect(screen.getByLabelText("Capture raw audio tags as separate observations")).not.toBeChecked();
  await userEvent.click(screen.getByRole("button",{name:"Choose library"}));
  await userEvent.click(screen.getByRole("button",{name:"Choose local mapping folder"}));
  expect(await screen.findByText("Selected local folder: /example/music")).toBeVisible();
  expect(screen.getByRole("button",{name:"Inspect library"})).toBeDisabled();
  await userEvent.type(screen.getByLabelText("Recorded source prefix"),"D:\\Music");
  await userEvent.click(screen.getByRole("button",{name:"Inspect library"}));
  await waitFor(()=>expect(api.inspectImport).toHaveBeenCalledOnce());
  expect(vi.mocked(api.inspectImport).mock.lastCall![1].path_mappings).toEqual([{source_prefix:"D:\\Music",root_id:"local-mapping"}]);
  expect(vi.mocked(api.inspectImport).mock.lastCall![1]).toMatchObject({include_history:false,include_queue:false,capture_file_tags:false});
  await userEvent.selectOptions(await screen.findByLabelText("Browse"),"collection");
  expect(await screen.findByText("2 inline occurrences; repeated tracks are retained")).toBeVisible();
 });
});
