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
 const names=["Casing","Characters to spaces","Encoded characters","Extract artist","Extract remixer","Garbage","Number prefix","URLs and email","Mix parentheses","Common title text","Promotional text"];
 for(const name of names){fireEvent.change(screen.getByLabelText("New Smart Fix"),{target:{value:name}});fireEvent.click(screen.getByRole("button",{name:"Add Smart Fix"}));}
 expect(screen.getAllByRole("group",{name:/Smart Fix:/})).toHaveLength(11);
 const casing=screen.getByRole("group",{name:"Smart Fix: Casing"});
 fireEvent.change(within(casing).getByLabelText("Field"),{target:{value:"artist"}});
 fireEvent.click(within(casing).getByLabelText("Uppercase values"));
 fireEvent.change(within(casing).getByLabelText("Ignored values, one per line"),{target:{value:"DJ TEST\nAC/DC"}});
 fireEvent.click(within(casing).getByRole("button",{name:"Move down"}));
 const options=(name:string)=>within(screen.getByRole("group",{name:"Smart Fix: "+name}));
 fireEvent.change(options("Characters to spaces").getByLabelText("Characters"),{target:{value:"_^"}});
 fireEvent.change(options("Extract artist").getByLabelText("Separator"),{target:{value:" — "}});
 fireEvent.change(options("Extract artist").getByLabelText("Result number"),{target:{value:"2"}});
 fireEvent.change(options("Extract remixer").getByLabelText("Remixer suffix"),{target:{value:"edit"}});
 fireEvent.click(options("Extract remixer").getByLabelText("Remove remixer from title"));
 fireEvent.change(options("Garbage").getByLabelText("Field"),{target:{value:"comment"}});
 fireEvent.click(options("URLs and email").getByLabelText("Clear entire matching field"));
 fireEvent.change(options("Common title text").getByLabelText("Pattern"),{target:{value:"BpmFromTitle"}});
 fireEvent.change(options("Promotional text").getByLabelText("Field"),{target:{value:"label"}});
 fireEvent.click(screen.getByLabelText("Clean incoming tracks automatically"));
 fireEvent.click(screen.getByRole("button",{name:"Save settings"}));
 await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
 const saved=vi.mocked(bridge.saveSettings).mock.calls.at(-1)![0];
 expect(saved.cleanup_enabled).toBe(true);expect(saved.cleanup!.fixes).toHaveLength(11);
 expect(saved.cleanup!.fixes[1].rule).toMatchObject({kind:"title_case",field:"artist",uppercase:false});
 expect(saved.cleanup!.fixes[1].ignored_values).toEqual(["DJ TEST","AC/DC"]);
 const rules=saved.cleanup!.fixes.map(f=>f.rule);
 expect(rules).toEqual(expect.arrayContaining([{kind:"replace_characters",characters:"_^"},{kind:"extract_artist",separator:" — ",part:2},{kind:"extract_remixer",word:"edit",remove_title:false},{kind:"remove_garbage",field:"comment"},{kind:"remove_urls",field:"title",clear_field:true},{kind:"remove_common_text",pattern:"BpmFromTitle"},{kind:"remove_promotional_text",field:"label"}]));
 expect(document.querySelector(".settings-body")).toBeInTheDocument();
});

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
