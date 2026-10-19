import {render,screen,fireEvent,waitFor,within} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import {customRow,cueTemplateError,factoryTemplate,tempoId,tempoPresets} from "../cue-template";

async function openCueSettings(){
  const bridge=fakeBridge([track("a")]);
  render(<App bridge={bridge}/>);
  await screen.findByText("Track a");
  fireEvent.click(screen.getByRole("button",{name:"Settings"}));
  await screen.findByRole("heading",{name:"Settings"});
  fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Cue generation"}));
  return bridge;
}
const save=()=>fireEvent.click(screen.getByRole("button",{name:"Save settings"}));
const lastSaved=(bridge:ReturnType<typeof fakeBridge>)=>vi.mocked(bridge.saveSettings).mock.calls.at(-1)![0];

it("shows the factory template and saves nothing extra until it is edited",async()=>{
  const bridge=await openCueSettings();
  const rows=within(screen.getByRole("table",{name:"Cue template rows"}));
  expect(rows.getAllByRole("row")).toHaveLength(7);
  expect(rows.getByLabelText("Name of Drop 2")).toHaveValue("Drop");
  save();await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
  expect(lastSaved(bridge).engine.cue_template ?? null).toBeNull();
  expect(lastSaved(bridge).engine.lexicon_tempo ?? null).toBeNull();
});

it("adds a custom cue before a drop and saves it as a template row",async()=>{
  const bridge=await openCueSettings();
  fireEvent.click(screen.getByRole("button",{name:"Add custom cue"}));
  fireEvent.change(screen.getByLabelText("Name of Custom cue 7"),{target:{value:"Pre-drop"}});
  fireEvent.change(screen.getByLabelText("Beats for custom cue 7"),{target:{value:"32"}});
  fireEvent.change(screen.getByLabelText("Colour of Custom cue 7"),{target:{value:"aqua"}});
  save();await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
  const template=lastSaved(bridge).engine.cue_template!;
  expect(template.rows).toHaveLength(7);
  expect(template.rows[6]).toMatchObject({markerType:"normal",name:"Pre-drop",anchor:"drop",position:"before",beatsOffset:32,cueType:"hotcue",color:"aqua",checked:true});
  expect(template.rows.slice(0,6)).toEqual(factoryTemplate().rows);
});

it("requires a loop length for loop rows and blocks saving a template the engine would refuse",async()=>{
  const bridge=await openCueSettings();
  fireEvent.click(screen.getByRole("button",{name:"Add custom cue"}));
  fireEvent.change(screen.getByLabelText("Name of Custom cue 7"),{target:{value:""}});
  expect(screen.getAllByRole("alert")[0]).toHaveTextContent("Cue row 7 needs a name");
  expect(screen.getByRole("button",{name:"Save settings"})).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Name of Custom cue 7"),{target:{value:"Loop"}});
  fireEvent.change(screen.getByLabelText("Type of Custom cue 7"),{target:{value:"loop_active"}});
  const length=screen.getByLabelText("Loop length for custom cue 7");
  expect(length).toHaveValue(8);
  fireEvent.change(length,{target:{value:"0"}});
  expect(screen.getAllByRole("alert")[0]).toHaveTextContent("needs a length in beats");
  fireEvent.change(length,{target:{value:"4"}});
  expect(screen.queryByRole("alert")).toBeNull();
  save();await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
  expect(lastSaved(bridge).engine.cue_template!.rows[6]).toMatchObject({cueType:"loop_active",loopDuration:4});
});

it("restores the factory template, which is saved as no custom template",async()=>{
  const bridge=await openCueSettings();
  fireEvent.click(screen.getByLabelText("Include Breakdown 3"));
  expect(screen.getByRole("button",{name:/Restore factory template/})).toBeEnabled();
  fireEvent.click(screen.getByRole("button",{name:/Restore factory template/}));
  expect(screen.getByLabelText("Include Breakdown 3")).toBeChecked();
  save();await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
  expect(lastSaved(bridge).engine.cue_template ?? null).toBeNull();
});

it("saves custom anchor names only for roles that have one",async()=>{
  const bridge=await openCueSettings();
  expect(screen.queryByLabelText("Drop anchor cue name")).toBeNull();
  fireEvent.click(screen.getByLabelText("Use my cues as anchors"));
  fireEvent.change(screen.getByLabelText("Drop anchor cue name"),{target:{value:"My Drop"}});
  fireEvent.change(screen.getByLabelText("Breakdown anchor cue name"),{target:{value:"Calm"}});
  fireEvent.change(screen.getByLabelText("Breakdown anchor cue name"),{target:{value:""}});
  save();await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
  const template=lastSaved(bridge).engine.cue_template!;
  expect(template.enableCustomAnchors).toBe(true);
  expect(template.customAnchors).toEqual({drop:{name:"My Drop"}});
});

it("offers the emergency loop row only while the detector setting is on",async()=>{
  const bridge=await openCueSettings();
  expect(screen.queryByLabelText(/Name of Emergency loop/)).toBeNull();
  fireEvent.click(screen.getByRole("checkbox",{name:/Emergency loop/}));
  const name=screen.getByLabelText("Name of Emergency loop 7");
  expect(name).toHaveValue("S.O.S.");
  fireEvent.change(name,{target:{value:"Escape"}});
  fireEvent.change(screen.getByLabelText("Loop length of Emergency loop 7"),{target:{value:"8"}});
  save();await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
  const saved=lastSaved(bridge);
  expect(saved.engine.lexicon_config!.add_emergency_loop).toBe(true);
  expect(saved.engine.cue_template!.rows[6]).toMatchObject({markerType:"emergency_loop",name:"Escape",loopDuration:8,cueType:"loop_active"});
});

it("chooses a tempo range from the reference presets and keeps the factory range unset",async()=>{
  const bridge=await openCueSettings();
  const select=screen.getByLabelText("Tempo range");
  expect(select).toHaveValue("98_195");
  expect(within(select).getAllByRole("option").map(o=>o.textContent)).toEqual(["Any","58 – 115","68 – 135","78 – 155","88 – 175","98 – 195 (default)","128 – 255","180+"]);
  fireEvent.change(select,{target:{value:"78_155"}});
  save();await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
  expect(lastSaved(bridge).engine.lexicon_tempo).toEqual({min_tempo:78,max_tempo:155});
  // Saving refreshes the draft from the stored settings; wait for that before editing again.
  await screen.findByRole("button",{name:/Settings saved/});
  await waitFor(()=>expect(screen.getByLabelText("Tempo range")).toHaveValue("78_155"));
  fireEvent.change(screen.getByLabelText("Tempo range"),{target:{value:"98_195"}});
  save();await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(2));
  expect(lastSaved(bridge).engine.lexicon_tempo ?? null).toBeNull();
});

it("validates templates with the same rules as the engine",()=>{
  const template=factoryTemplate();
  expect(cueTemplateError(template)).toBeNull();
  expect(cueTemplateError({...template,rows:[...template.rows,customRow(),customRow()]})).toMatch(/same cue/);
  expect(cueTemplateError({...template,rows:[...template.rows,{...customRow(),beatsOffset:1.5}]})).toMatch(/whole number/);
  expect(cueTemplateError({...template,rows:[...template.rows,{...customRow(),cueType:"loop_hot"}]})).toMatch(/length in beats/);
  expect(cueTemplateError({...template,rows:[...template.rows,{...customRow(),checked:false},customRow()]})).toBeNull();
  expect(cueTemplateError({...template,enableCustomAnchors:true,customAnchors:{drop:{name:" "}}})).toMatch(/needs a cue name/);
  expect(tempoId(null)).toBe("98_195");
  expect(tempoId({min_tempo:100,max_tempo:150})).toBe("custom");
  expect(tempoPresets.every(p=>tempoId(p.range)===p.id)).toBe(true);
});
