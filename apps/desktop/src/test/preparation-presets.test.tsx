import {act,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import PreparationPresetsSettings from "../views/PreparationPresetsSettings";
import {fakeBridge} from "./bridge";
import type {PreparationPreset} from "../preset-types";
import type {Settings} from "../types";
async function fixture(){const bridge=fakeBridge([]),settings=(await bridge.getState()).settings.settings;const preset:PreparationPreset={id:"late",revision:"p1",name:"Late night",status:"ready",issues:[],assigned_folder_count:0,connections:{destination_root_id:null},rules:{cue_generation:{genre:null,drop_at_start:"Never",min_cue_length_beats:64,add_emergency_loop:false},autotag_enabled:false,tagging:null,cleanup_enabled:false,cleanup:null,organization:{mode:"leave_in_place",pattern:"{title}",missing:"block",collision:"block",organization_optional:false}}};bridge.preparationPresets=vi.fn(async()=>[preset]);bridge.preparationPreset=vi.fn(async()=>preset);return {bridge,settings,preset};}
describe("preparation presets",()=>{
 it("refreshes a changed selected preset revision without replacing the global draft",async()=>{const {bridge,settings,preset}=await fixture(),changed=vi.fn();let notify!:()=>void;bridge.onPreparationPresetsChange=vi.fn(async handler=>{notify=handler;return()=>{};});const view=render(<PreparationPresetsSettings bridge={bridge} settings={settings} onDraftChange={changed}/>);await waitFor(()=>expect(screen.getByLabelText("Preset name")).toHaveValue("Late night"));const next={...preset,revision:"p2",name:"After hours"};bridge.preparationPresets=vi.fn(async()=>[next]);bridge.preparationPreset=vi.fn(async()=>next);act(()=>notify());await waitFor(()=>expect(screen.getByLabelText("Preset name")).toHaveValue("After hours"));expect(changed).not.toHaveBeenCalled();expect(bridge.preparationPreset).toHaveBeenLastCalledWith("late");view.unmount();});
 it("refuses a late overlay that would overwrite newer unsaved global settings",async()=>{
  const {bridge,settings}=await fixture(),changed=vi.fn();let resolve!:(value:Settings)=>void;
  bridge.overlayPreparationPreset=vi.fn(()=>new Promise<Settings>(done=>{resolve=done;}));
  const view=render(<PreparationPresetsSettings bridge={bridge} settings={settings} onDraftChange={changed}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Load into settings draft"}));
  await waitFor(()=>expect(bridge.overlayPreparationPreset).toHaveBeenCalled());
  const newer={...settings,stability_millis:4321};view.rerender(<PreparationPresetsSettings bridge={bridge} settings={newer} onDraftChange={changed}/>);
  await act(async()=>resolve({...settings,cleanup_enabled:true}));
  expect(await screen.findByRole("alert")).toHaveTextContent("Settings draft changed");expect(changed).not.toHaveBeenCalled();
 });
 it("shows native import blockers and never imports transport-bearing documents",async()=>{
  const {bridge,settings,preset}=await fixture(),document='{"solver":{"endpoint":"http://example.invalid"}}';
  bridge.choosePreparationPresetImport=vi.fn(async()=>document);bridge.previewPreparationPresetImport=vi.fn(async()=>({name:"Imported",rules:preset.rules,status:"unsupported",issues:[{code:"transport",field:"solver",message:"Transport fields are not portable"}]}));
  render(<PreparationPresetsSettings bridge={bridge} settings={settings} onDraftChange={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Import preset file"}));
  await screen.findByText("Transport fields are not portable");expect(screen.getByRole("button",{name:"Import reviewed preset"})).toBeDisabled();expect(bridge.importPreparationPreset).not.toHaveBeenCalled();
 });
 it("reconnects using an opaque native root ID and the captured preset revision",async()=>{
  const {bridge,settings,preset}=await fixture();bridge.chooseOrganizationRoot=vi.fn(async()=>({id:"native-root",display_path:"/display/only"}));bridge.savePreparationPreset=vi.fn(async request=>({...preset,...request,id:preset.id,revision:"p2",expected_revision:undefined}));
  render(<PreparationPresetsSettings bridge={bridge} settings={settings} onDraftChange={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Reconnect destination"}));
  await waitFor(()=>expect(bridge.savePreparationPreset).toHaveBeenCalledWith({id:"late",expected_revision:"p1",name:"Late night",rules:preset.rules,connections:{destination_root_id:"native-root"}}));
 });
});
