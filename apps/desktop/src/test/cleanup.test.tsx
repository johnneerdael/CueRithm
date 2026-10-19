import {render,screen,fireEvent,waitFor,within,act} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import type {CleanupPreview,LibraryChangeEvent} from "../types";

const preview:CleanupPreview={id:"plan",created:1,unchanged:[],failed:[],proposals:["a","b"].map(id=>({id:"p-"+id,track_id:id,title:"Track "+id,report:{original:{fields:{title:["OLD "+id]},custom_tags:[]},metadata:{fields:{title:["New "+id]},custom_tags:[]},changes:[{field:"title",before:["OLD "+id],after:["New "+id]}],applied_rules:["fix:one"]}}))};
async function openCleanup(bridge:ReturnType<typeof fakeBridge>){render(<App bridge={bridge}/>);await screen.findByText("Track a");fireEvent.click(screen.getByRole("button",{name:"Cleanup"}));await screen.findByRole("heading",{name:"Cleanup"});}

it("exposes every Smart Fix and persistent ordered options in Settings",async()=>{
 const bridge=fakeBridge([]);render(<App bridge={bridge}/>);
 fireEvent.click(screen.getByRole("button",{name:"Settings"}));
 await screen.findByRole("heading",{name:"Settings"});
 fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Cleanup"}));
 const cleanup=within(screen.getByRole("heading",{name:"Cleanup"}).closest("section")!);
 const names=["Casing","Characters to spaces","Encoded characters","Extract artist","Extract remixer","Garbage","Number prefix","URLs and email","Mix parentheses","Common title text","Promotional text"];
 for(const name of names){fireEvent.change(cleanup.getByLabelText("New Smart Fix"),{target:{value:name}});fireEvent.click(cleanup.getByRole("button",{name:"Add Smart Fix"}));}
 expect(cleanup.getAllByRole("group",{name:/Smart Fix:/})).toHaveLength(11);
 const casing=cleanup.getByRole("group",{name:"Smart Fix: Casing"});
 fireEvent.change(within(casing).getByLabelText("Field"),{target:{value:"artist"}});
 fireEvent.click(within(casing).getByLabelText("Uppercase values"));
 fireEvent.change(within(casing).getByLabelText("Ignored values, one per line"),{target:{value:"DJ TEST\nAC/DC"}});
 fireEvent.click(within(casing).getByRole("button",{name:"Move down"}));
 const options=(name:string)=>within(cleanup.getByRole("group",{name:"Smart Fix: "+name}));
 fireEvent.change(options("Characters to spaces").getByLabelText("Characters"),{target:{value:"_^"}});
 fireEvent.change(options("Extract artist").getByLabelText("Separator"),{target:{value:" — "}});
 fireEvent.change(options("Extract artist").getByLabelText("Result number"),{target:{value:"2"}});
 fireEvent.change(options("Extract remixer").getByLabelText("Remixer suffix"),{target:{value:"edit"}});
 fireEvent.click(options("Extract remixer").getByLabelText("Remove remixer from title"));
 fireEvent.change(options("Garbage").getByLabelText("Field"),{target:{value:"comment"}});
 fireEvent.click(options("URLs and email").getByLabelText("Clear entire matching field"));
 fireEvent.change(options("Common title text").getByLabelText("Pattern"),{target:{value:"BpmFromTitle"}});
 fireEvent.change(options("Promotional text").getByLabelText("Field"),{target:{value:"label"}});
 fireEvent.click(cleanup.getByLabelText("Clean incoming tracks automatically"));
 fireEvent.click(screen.getByRole("button",{name:"Save settings"}));
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
 const saved=vi.mocked(bridge.saveSettings).mock.calls.at(-1)![0];
 expect(saved.cleanup_enabled).toBe(true);expect(saved.cleanup!.fixes).toHaveLength(11);
 expect(saved.cleanup!.fixes[1].rule).toMatchObject({kind:"title_case",field:"artist",uppercase:false});
 expect(saved.cleanup!.fixes[1].ignored_values).toEqual(["DJ TEST","AC/DC"]);
 const rules=saved.cleanup!.fixes.map(f=>f.rule);
 expect(rules).toEqual(expect.arrayContaining([{kind:"replace_characters",characters:"_^"},{kind:"extract_artist",separator:" — ",part:2},{kind:"extract_remixer",word:"edit",remove_title:false},{kind:"remove_garbage",field:"comment"},{kind:"remove_urls",field:"title",clear_field:true},{kind:"remove_common_text",pattern:"BpmFromTitle"},{kind:"remove_promotional_text",field:"label"}]));
 expect(document.querySelector(".settings-body")).toBeInTheDocument();
},15_000);

it("scans without applying, applies only checked proposals, reports conflicts and reloads durable Undo history",async()=>{
 const bridge=fakeBridge([track("a"),track("b")]);bridge.previewCleanup=vi.fn(async()=>preview);
 bridge.applyCleanup=vi.fn(async()=>({id:"batch",applied:["a"],unchanged:[],conflicted:[{track_id:"b",message:"Source changed; rescan"}],failed:[]}));
 const created=Date.UTC(2026,9,18,10,30);
 bridge.cleanupHistory=vi.fn(async()=>[{can_undo:true,id:"batch",kind:"apply",state:"applied",created,result:{id:"batch",applied:["a"],unchanged:[],conflicted:[],failed:[]}}]);
 await openCleanup(bridge);
 expect(screen.getByText(new RegExp(new Date(created).toLocaleString().replace(/[.*+?^${}()|[\]\\]/g,"\\$&")))).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Scan tracks"}));await screen.findByText("OLD a");
 expect(bridge.applyCleanup).not.toHaveBeenCalled();
 fireEvent.click(screen.getByLabelText("Apply cleanup to Track b"));
 fireEvent.click(screen.getByRole("button",{name:"Apply 1 change"}));
 await waitFor(()=>expect(bridge.applyCleanup).toHaveBeenCalledWith("plan",["p-a"]));
 await screen.findByText("Source changed; rescan");
 fireEvent.click(screen.getByRole("button",{name:"Undo batch"}));await waitFor(()=>expect(bridge.undoCleanup).toHaveBeenCalledWith("batch"));
 expect(bridge.playerLoad).not.toHaveBeenCalled();
});

it("shows counted native genres and contributor roles with protected exact mappings",async()=>{
 const bridge=fakeBridge([track("a")]);bridge.cleanupCatalog=vi.fn(async()=>({genres:[{value:"Deep House",track_count:2,protected:false,mapping:null}],artists:[{value:"Earth, Wind & Fire",track_count:3,roles:{artist:2,producer:1},protected:false,mapping:null}],categories:[],unavailable_tracks:[]}));
 await openCleanup(bridge);
 fireEvent.click(screen.getByRole("tab",{name:"Genres"}));
 await screen.findByText("Deep House");expect(screen.getByText("2 tracks")).toBeInTheDocument();
 fireEvent.change(screen.getByLabelText("Replace Deep House with"),{target:{value:"House"}});
 fireEvent.click(screen.getByLabelText("Protect Deep House"));
 fireEvent.click(screen.getByRole("button",{name:"Save cleanup rules"}));
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
 expect(vi.mocked(bridge.saveSettings).mock.calls.at(-1)![0].cleanup!.genres).toMatchObject({mappings:[{from:"Deep House",to:"House"}],protected_values:["Deep House"]});
 fireEvent.click(screen.getByRole("tab",{name:"Artists"}));await screen.findByText("Earth, Wind & Fire");
 expect(screen.getByText("artist: 2 · producer: 1")).toBeInTheDocument();
});

it("invalidates stale previews after rule edits and ignores late scan results after leaving the view",async()=>{
 const bridge=fakeBridge([track("a")]);let resolve!:(v:CleanupPreview)=>void;
 bridge.previewCleanup=vi.fn(()=>new Promise<CleanupPreview>(r=>{resolve=r;}));await openCleanup(bridge);
 fireEvent.click(screen.getByRole("button",{name:"Scan tracks"}));expect(screen.getByRole("status")).toHaveTextContent("Scanning");
 fireEvent.click(screen.getByRole("button",{name:/^Tracks/}));await screen.findByText("Track a");
 await act(async()=>resolve(preview));expect(screen.queryByText("OLD a")).not.toBeInTheDocument();
});

it("refreshes a playing track after manual cleanup without restarting or loading audio",async()=>{
 const bridge=fakeBridge([track("a")]);let changed!:(e:LibraryChangeEvent)=>void;
 bridge.onLibraryChange=vi.fn(async handler=>{changed=handler;return()=>{};});
 render(<App bridge={bridge}/>);await screen.findByText("Track a");fireEvent.click(screen.getByRole("button",{name:"Play Track a"}));
 const player=screen.getByRole("region",{name:"Music player"});await waitFor(()=>expect(within(player).getByText("Track a")).toBeInTheDocument());
 bridge.getTrack=vi.fn(async()=>({track:{...track("a"),title:"Clean title"},analysis:null,source_sha256:"newhash",pcm_sha256:"pcm"}));
 act(()=>changed({track_ids:["a"]}));await within(player).findByText("Clean title");expect(bridge.playerLoad).toHaveBeenCalledTimes(1);expect(bridge.playerPause).not.toHaveBeenCalled();
});

it("captures the table selection as cleanup scope and clears a preview when rules change",async()=>{
 const bridge=fakeBridge([track("a"),track("b")]);bridge.previewCleanup=vi.fn(async()=>preview);
 render(<App bridge={bridge}/>);fireEvent.click(await screen.findByText("Track a"));
 fireEvent.click(screen.getByRole("button",{name:"Cleanup"}));await screen.findByRole("heading",{name:"Cleanup"});
 fireEvent.click(screen.getByRole("button",{name:"Scan tracks"}));await screen.findByText("OLD a");
 expect(bridge.previewCleanup).toHaveBeenCalledWith({kind:"track_ids",track_ids:["a"]},expect.any(Object));
 fireEvent.click(screen.getByRole("button",{name:"Add Smart Fix"}));
 expect(screen.queryByText("OLD a")).not.toBeInTheDocument();expect(screen.getByRole("button",{name:"Scan tracks"})).toBeDisabled();
});

it("reports empty scans and recoverable errors without publishing a failed preview",async()=>{
 const bridge=fakeBridge([track("a")]);await openCleanup(bridge);
 fireEvent.click(screen.getByRole("button",{name:"Scan tracks"}));await screen.findByText("No metadata changes needed with these rules.");
 bridge.previewCleanup=vi.fn(async()=>{throw new Error("Source unavailable");});
 fireEvent.click(screen.getByRole("button",{name:"Scan tracks"}));await screen.findByRole("alert");
 expect(screen.getByRole("alert")).toHaveTextContent("Source unavailable");expect(screen.queryByRole("button",{name:/^Apply/})).not.toBeInTheDocument();
});

it("retains identifiable conflict and failure details in history after reopening Cleanup",async()=>{
 const bridge=fakeBridge([track("a")]);
 bridge.cleanupHistory=vi.fn(async()=>[{can_undo:true,id:"old-batch",kind:"apply",state:"interrupted",created:Date.now(),result:{id:"old-batch",applied:["a"],unchanged:[],conflicted:[{track_id:"newer-edit",message:"Newer metadata preserved"}],failed:[{track_id:"offline-file",message:"Source disk unavailable"}]}}]);
 await openCleanup(bridge);
 fireEvent.click(screen.getByText("1 conflict · 1 failed"));
 expect(screen.getByText("newer-edit")).toBeInTheDocument();expect(screen.getByText("Newer metadata preserved")).toBeInTheDocument();
 expect(screen.getByText("offline-file")).toBeInTheDocument();expect(screen.getByText("Source disk unavailable")).toBeInTheDocument();
});

const textFixes=[
 {name:"Characters to spaces",label:"Characters",property:"characters"},
 {name:"Extract artist",label:"Separator",property:"separator"},
 {name:"Extract remixer",label:"Remixer suffix",property:"word"},
] as const;
async function openTextFix(name:string){
 const bridge=fakeBridge([]);render(<App bridge={bridge}/>);
 fireEvent.click(screen.getByRole("button",{name:"Settings"}));
 await screen.findByRole("heading",{name:"Settings"});
 fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Cleanup"}));
 fireEvent.change(screen.getByLabelText("New Smart Fix"),{target:{value:name}});
 fireEvent.click(screen.getByRole("button",{name:"Add Smart Fix"}));
 return bridge;
}
it.each(textFixes.flatMap(fix=>["x","😀"].map(character=>({...fix,character}))))("validates $label by Unicode character count for $character without truncating",async({name,label,property,character})=>{
 const bridge=await openTextFix(name),input=screen.getByLabelText(label);
 fireEvent.change(input,{target:{value:character.repeat(65)}});
 expect(input).toHaveValue(character.repeat(65));
 expect(input).toHaveAttribute("aria-invalid","true");
 expect(input).toHaveAccessibleDescription(/65\/64 characters/);
 expect(screen.getByRole("button",{name:"Save settings"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Save settings"}));
 expect(bridge.saveSettings).not.toHaveBeenCalled();
 fireEvent.change(input,{target:{value:character.repeat(64)}});
 expect(input).toHaveValue(character.repeat(64));
 expect(input).not.toHaveAttribute("aria-invalid","true");
 expect(input).toHaveAccessibleDescription(/64\/64 characters/);
 fireEvent.click(screen.getByRole("button",{name:"Save settings"}));
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
 expect(vi.mocked(bridge.saveSettings).mock.calls[0][0].cleanup!.fixes[0].rule).toMatchObject({[property]:character.repeat(64)});
});
it.each(textFixes)("uses the backend empty-value policy for $label",async({name,label})=>{
 const bridge=await openTextFix(name),input=screen.getByLabelText(label);
 fireEvent.change(input,{target:{value:""}});
 const save=screen.getByRole("button",{name:"Save settings"});
 if(label==="Characters"){
  expect(save).toBeEnabled();fireEvent.click(save);await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
 }else{
  expect(input).toHaveAttribute("aria-invalid","true");expect(save).toBeDisabled();
  fireEvent.change(input,{target:{value:" "}});
  if(label==="Separator")expect(save).toBeEnabled();else expect(save).toBeDisabled();
 }
});
it("matches Rust Unicode whitespace semantics for the remixer suffix",async()=>{
 await openTextFix("Extract remixer");
 const input=screen.getByLabelText("Remixer suffix"),save=screen.getByRole("button",{name:"Save settings"});
 // Rust White_Space includes NEXT LINE; JavaScript trim does not.
 fireEvent.change(input,{target:{value:"\u0085"}});expect(save).toBeDisabled();
 // BOM is not Unicode White_Space, even though JavaScript trim removes it.
 fireEvent.change(input,{target:{value:"\ufeff"}});expect(save).toBeEnabled();
});
it("keeps invalid Cleanup drafts unsavable after changing Settings pages",async()=>{
 const bridge=await openTextFix("Characters to spaces");
 fireEvent.change(screen.getByLabelText("Characters"),{target:{value:"x".repeat(65)}});
 fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Incoming"}));
 expect(screen.getByRole("button",{name:"Save settings"})).toBeDisabled();
 expect(screen.getByText(/Characters.*64 characters/)).toBeInTheDocument();
 expect(bridge.saveSettings).not.toHaveBeenCalled();
});
it("blocks invalid manual cleanup rules until their Unicode text is corrected",async()=>{
 const bridge=fakeBridge([track("a")]);await openCleanup(bridge);
 fireEvent.change(screen.getByLabelText("New Smart Fix"),{target:{value:"Extract artist"}});
 fireEvent.click(screen.getByRole("button",{name:"Add Smart Fix"}));
 fireEvent.change(screen.getByLabelText("Separator"),{target:{value:"😀".repeat(65)}});
 expect(screen.getByRole("button",{name:"Save cleanup rules"})).toBeDisabled();
 expect(screen.getByRole("button",{name:"Scan tracks"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Save cleanup rules"}));expect(bridge.saveSettings).not.toHaveBeenCalled();
 fireEvent.change(screen.getByLabelText("Separator"),{target:{value:"😀".repeat(64)}});
 expect(screen.getByRole("button",{name:"Save cleanup rules"})).toBeEnabled();
 fireEvent.click(screen.getByRole("button",{name:"Save cleanup rules"}));
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
});

it.each(textFixes)("rejects invalid Unicode in $label without altering the draft",async({name,label})=>{
 await openTextFix(name);
 const input=screen.getByLabelText(label);
 fireEvent.change(input,{target:{value:"\ud800"}});
 expect(input).toHaveValue("\ud800");
 expect(input).toHaveAttribute("aria-invalid","true");
 expect(screen.getByRole("button",{name:"Save settings"})).toBeDisabled();
 fireEvent.change(input,{target:{value:"😀"}});
 expect(screen.getByRole("button",{name:"Save settings"})).toBeEnabled();
});

it.each(textFixes)("lets disabled $label drafts save and blocks them again when enabled",async({name,label})=>{
 const bridge=await openTextFix(name),input=screen.getByLabelText(label);
 fireEvent.change(input,{target:{value:"x".repeat(65)}});
 const save=screen.getByRole("button",{name:"Save settings"}),enabled=within(screen.getByRole("group",{name:"Smart Fix: "+name})).getByLabelText("Enabled");
 expect(save).toBeDisabled();
 fireEvent.click(enabled);expect(save).toBeEnabled();
 fireEvent.click(enabled);expect(save).toBeDisabled();
 fireEvent.click(enabled);fireEvent.click(save);
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
 expect(vi.mocked(bridge.saveSettings).mock.calls[0][0].cleanup!.fixes[0]).toMatchObject({enabled:false});
});
it("allows manual cleanup to save an invalid disabled rule without truncation",async()=>{
 const bridge=fakeBridge([track("a")]);await openCleanup(bridge);
 fireEvent.change(screen.getByLabelText("New Smart Fix"),{target:{value:"Extract artist"}});
 fireEvent.click(screen.getByRole("button",{name:"Add Smart Fix"}));
 fireEvent.change(screen.getByLabelText("Separator"),{target:{value:"😀".repeat(65)}});
 const save=screen.getByRole("button",{name:"Save cleanup rules"}),enabled=within(screen.getByRole("group",{name:"Smart Fix: Extract artist"})).getByLabelText("Enabled");
 expect(save).toBeDisabled();fireEvent.click(enabled);expect(save).toBeEnabled();
 fireEvent.click(enabled);expect(save).toBeDisabled();fireEvent.click(enabled);fireEvent.click(save);
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
 expect(vi.mocked(bridge.saveSettings).mock.calls[0][0].cleanup!.fixes[0]).toMatchObject({enabled:false,rule:{separator:"😀".repeat(65)}});
});

async function openResultNumber(manual:boolean){
 if(!manual)return openTextFix("Extract artist");
 const bridge=fakeBridge([track("a")]);await openCleanup(bridge);
 fireEvent.change(screen.getByLabelText("New Smart Fix"),{target:{value:"Extract artist"}});
 fireEvent.click(screen.getByRole("button",{name:"Add Smart Fix"}));return bridge;
}
const resultEditors=[{manual:false,saveLabel:"Save settings"},{manual:true,saveLabel:"Save cleanup rules"}];
it.each(resultEditors.flatMap(editor=>["","0","-1","257","1.5"].map(value=>({...editor,value}))))("blocks result number '$value' in $saveLabel",async({manual,saveLabel,value})=>{
 const bridge=await openResultNumber(manual),input=screen.getByLabelText("Result number");
 fireEvent.change(input,{target:{value}});
 expect(input).toHaveAttribute("aria-invalid","true");
 expect(input).toHaveAccessibleDescription(/whole number from 1 to 256/);
 const save=screen.getByRole("button",{name:saveLabel});expect(save).toBeDisabled();fireEvent.click(save);
 expect(bridge.saveSettings).not.toHaveBeenCalled();
 if(manual)expect(screen.getByRole("button",{name:"Scan tracks"})).toBeDisabled();
});
it.each(resultEditors.flatMap(editor=>[1,256].map(part=>({...editor,part}))))("saves result number boundary $part in $saveLabel",async({manual,saveLabel,part})=>{
 const bridge=await openResultNumber(manual);
 fireEvent.change(screen.getByLabelText("Result number"),{target:{value:String(part)}});
 expect(screen.getByRole("button",{name:saveLabel})).toBeEnabled();fireEvent.click(screen.getByRole("button",{name:saveLabel}));
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
 expect(vi.mocked(bridge.saveSettings).mock.calls[0][0].cleanup!.fixes[0].rule).toMatchObject({part});
});
it.each(resultEditors.flatMap(editor=>[0,257].map(part=>({...editor,part}))))("saves disabled result number $part but blocks re-enabling it in $saveLabel",async({manual,saveLabel,part})=>{
 const bridge=await openResultNumber(manual),input=screen.getByLabelText("Result number"),enabled=within(screen.getByRole("group",{name:"Smart Fix: Extract artist"})).getByLabelText("Enabled");
 fireEvent.change(input,{target:{value:String(part)}});const save=screen.getByRole("button",{name:saveLabel});expect(save).toBeDisabled();
 fireEvent.click(enabled);expect(save).toBeEnabled();fireEvent.click(enabled);expect(save).toBeDisabled();
 fireEvent.click(enabled);fireEvent.click(save);await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
 expect(vi.mocked(bridge.saveSettings).mock.calls[0][0].cleanup!.fixes[0]).toMatchObject({enabled:false,rule:{part}});
});
it.each(resultEditors)("rejects result numbers that Rust cannot deserialize even when disabled in $saveLabel",async({manual,saveLabel})=>{
 const bridge=await openResultNumber(manual),input=screen.getByLabelText("Result number"),enabled=within(screen.getByRole("group",{name:"Smart Fix: Extract artist"})).getByLabelText("Enabled");
 fireEvent.click(enabled);
 for(const value of ["-1","1.5","65536","1e309"]){
  fireEvent.change(input,{target:{value}});
  expect(screen.getByRole("button",{name:saveLabel})).toBeDisabled();
  expect(input).toHaveAttribute("aria-invalid","true");
  fireEvent.click(screen.getByRole("button",{name:saveLabel}));expect(bridge.saveSettings).not.toHaveBeenCalled();
 }
});

async function openCanonicalRules(manual:boolean,kind:"genres"|"artists"){
 let bridge:ReturnType<typeof fakeBridge>;
 if(manual){bridge=fakeBridge([track("a")]);await openCleanup(bridge);fireEvent.click(screen.getByRole("tab",{name:kind==="genres"?"Genres":"Artists"}));}
 else{bridge=fakeBridge([]);render(<App bridge={bridge}/>);fireEvent.click(screen.getByRole("button",{name:"Settings"}));await screen.findByRole("heading",{name:"Settings"});fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Cleanup"}));}
 const checkbox=await screen.findByLabelText(kind==="genres"?"Enable genre cleanup":"Enable artist cleanup");
 const editor=within(checkbox.closest(".canonical-editor")!);fireEvent.click(checkbox);
 return {bridge,editor,save:()=>screen.getByRole("button",{name:manual?"Save cleanup rules":"Save settings"})};
}
it.each(resultEditors.flatMap(editor=>["genres","artists"].map(kind=>({...editor,kind:kind as "genres"|"artists"}))))("explains invalid $kind mappings and recovers in $saveLabel",async({manual,kind})=>{
 const {bridge,editor,save}=await openCanonicalRules(manual,kind);
 fireEvent.click(editor.getByRole("button",{name:"Add mapping"}));expect(save()).toBeDisabled();
 expect(screen.getAllByRole("alert").some(alert=>/mapping 1.*source/i.test(alert.textContent??""))).toBe(true);
 fireEvent.click(save());expect(bridge.saveSettings).not.toHaveBeenCalled();
 fireEvent.change(editor.getByLabelText("Source value 1"),{target:{value:"Deep House"}});expect(save()).toBeDisabled();
 fireEvent.change(editor.getByLabelText("Canonical value 1"),{target:{value:"House"}});expect(save()).toBeEnabled();
 fireEvent.click(editor.getByRole("button",{name:"Add mapping"}));fireEvent.change(editor.getByLabelText("Source value 2"),{target:{value:"House"}});fireEvent.change(editor.getByLabelText("Canonical value 2"),{target:{value:" Deep House "}});
 expect(save()).toBeDisabled();expect(screen.getAllByRole("alert").some(alert=>/cycle/i.test(alert.textContent??""))).toBe(true);
 fireEvent.change(editor.getByLabelText("Canonical value 2"),{target:{value:"Tech House"}});expect(save()).toBeEnabled();fireEvent.click(save());
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
});
it.each(resultEditors)("explains missing artist roles in $saveLabel",async({manual})=>{
 const {bridge,editor,save}=await openCanonicalRules(manual,"artists");
 fireEvent.click(editor.getByLabelText("artist"));expect(save()).toBeDisabled();
 expect(screen.getAllByRole("alert").some(alert=>/contributor role/i.test(alert.textContent??""))).toBe(true);
 fireEvent.click(save());expect(bridge.saveSettings).not.toHaveBeenCalled();
 fireEvent.click(editor.getByLabelText("producer"));expect(save()).toBeEnabled();fireEvent.click(save());await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
});
it.each(resultEditors)("keeps invalid genre conversion errors visible across pages or tabs in $saveLabel",async({manual})=>{
 const {bridge,editor,save}=await openCanonicalRules(manual,"genres");
 fireEvent.click(editor.getByLabelText("Convert genres into categorized tags"));fireEvent.change(editor.getByLabelText("Genre tag category"),{target:{value:""}});expect(save()).toBeDisabled();
 expect(screen.getAllByRole("alert").some(alert=>/genre tag category/i.test(alert.textContent??""))).toBe(true);
 if(manual)fireEvent.click(screen.getByRole("tab",{name:"Artists"}));else fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Incoming"}));
 expect(save()).toBeDisabled();expect(screen.getAllByRole("alert").some(alert=>/genre tag category/i.test(alert.textContent??""))).toBe(true);
 fireEvent.click(save());expect(bridge.saveSettings).not.toHaveBeenCalled();
});

it.each(resultEditors)("preserves valid BOM values and drops Rust-whitespace lines in $saveLabel",async({manual})=>{
 const {bridge,editor,save}=await openCanonicalRules(manual,"genres");
 fireEvent.change(editor.getByLabelText("Protected genres, one per line"),{target:{value:"\ufeff\n\u0085\nHouse"}});
 expect(save()).toBeEnabled();fireEvent.click(save());await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
 expect(vi.mocked(bridge.saveSettings).mock.calls[0][0].cleanup!.genres.protected_values).toEqual(["\ufeff","House"]);
});
it.each(resultEditors)("preserves disabled invalid canonical drafts in $saveLabel",async({manual})=>{
 const {bridge,editor,save}=await openCanonicalRules(manual,"genres");
 fireEvent.click(editor.getByRole("button",{name:"Add mapping"}));
 fireEvent.click(editor.getByLabelText("Convert genres into categorized tags"));fireEvent.change(editor.getByLabelText("Genre tag category"),{target:{value:""}});
 expect(save()).toBeDisabled();fireEvent.click(editor.getByLabelText("Enable genre cleanup"));expect(save()).toBeEnabled();fireEvent.click(save());
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalledTimes(1));
 expect(vi.mocked(bridge.saveSettings).mock.calls[0][0].cleanup!.genres).toMatchObject({enabled:false,mappings:[{from:"",to:""}],conversion:{category:""}});
});
