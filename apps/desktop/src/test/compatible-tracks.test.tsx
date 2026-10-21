import {fireEvent,render,screen,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import CompatibleTracks from "../views/CompatibleTracks";
import type {CompatibleBridge,CompatibleTrackResult} from "../compatible-types";
import {fakeBridge,track} from "./bridge";
function result():CompatibleTrackResult{return {source:track("source"),key_mode:"fuzzy",matches:[{track:{...track("match"),energy:null},reasons:["Compatible key: 8A → 9B (fuzzy)","BPM: 128 × 1 = 128; difference 0 (±4)"],bpm_delta:0,bpm_multiplier:1,energy_delta:null,same_key:false}],checked:4,excluded_unknown:2,library_revision:1};}
function setup(find:CompatibleBridge["findCompatibleTracks"]=async()=>result(),configure?:(bridge:ReturnType<typeof fakeBridge>)=>void,onPreview?:(candidate:ReturnType<typeof track>)=>Promise<void>){
 const bridge=fakeBridge([track("source"),track("match")]);bridge.getPlaylistPreferences=vi.fn(async()=>({revision:0,preferences:{time_zone:null,key_mixing_mode:"fuzzy" as const}}));
 configure?.(bridge);
 const compatibleBridge:CompatibleBridge={findCompatibleTracks:vi.fn(find),saveCompatiblePlaylist:vi.fn(async()=>({edit_id:"compatible-save",revision:1,created_ids:["saved"],affected_ids:["saved"]}))},onSelect=vi.fn(),onSaved=vi.fn(async()=>{});
 render(<CompatibleTracks bridge={bridge} compatibleBridge={compatibleBridge} initialSourceId="source" onSelect={onSelect} onPreview={onPreview} onClose={()=>{}} onSaved={onSaved}/>);return {bridge,compatibleBridge,onSelect,onSaved};
}
describe("Compatible tracks",()=>{
 it("uses saved key mode and explicit defaults, then shows reasons and unknown measurements",async()=>{
 const {compatibleBridge}=setup();await screen.findByRole("button",{name:"Find compatible tracks"});await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());
 expect(screen.getByLabelText("Key mode")).toHaveValue("fuzzy");fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));
 await screen.findByText("Compatible key: 8A → 9B (fuzzy)");expect(screen.getByText("Energy unknown")).toBeInTheDocument();expect(screen.getByText(/2 tracks excluded/)).toBeInTheDocument();
 expect(compatibleBridge.findCompatibleTracks).toHaveBeenCalledWith({source_id:"source",compatible_key:true,key_mode:"fuzzy",bpm_tolerance:4,include_half_double:false,energy_tolerance:null,genre:null,custom_tag_ids:[]});
 });
 it("invalidates results and ignores late previews when controls change",async()=>{
 let resolve!:(r:CompatibleTrackResult)=>void;setup(()=>new Promise(r=>{resolve=r;}));await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));
 fireEvent.change(screen.getByLabelText("BPM tolerance"),{target:{value:"2"}});resolve(result());await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());expect(screen.queryByText("Compatible key: 8A → 9B (fuzzy)")).not.toBeInTheDocument();
 expect(screen.getByRole("button",{name:"Save selected to playlist"})).toBeDisabled();
 });
 it("auditions a candidate and saves ranked selections in one ordinary-playlist batch",async()=>{
 const {bridge,compatibleBridge,onSelect,onSaved}=setup();await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));await screen.findByText("Compatible key: 8A → 9B (fuzzy)");
 fireEvent.click(screen.getByRole("button",{name:"Preview Track match"}));await waitFor(()=>expect(bridge.playerPlay).toHaveBeenCalled());expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({id:"match"}));
 fireEvent.click(screen.getByLabelText("Select Track match"));fireEvent.change(screen.getByLabelText("Playlist name"),{target:{value:"Practice"}});fireEvent.click(screen.getByRole("button",{name:"Save selected to playlist"}));
 await screen.findByText(/Saved 1 track/);expect(bridge.editPlaylist).not.toHaveBeenCalled();expect(compatibleBridge.saveCompatiblePlaylist).toHaveBeenCalledWith({name:"Practice",track_ids:["match"],expected_library_revision:1,expected_playlist_revision:0});expect(onSaved).toHaveBeenCalledTimes(1);
 });
 it("shows actionable backend validation and rejects invalid tolerance before searching",async()=>{
 const {compatibleBridge}=setup(async()=>{throw Error("Source BPM is unknown. Prepare or correct the source, or disable BPM matching.");});await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());
 fireEvent.change(screen.getByLabelText("BPM tolerance"),{target:{value:"-1"}});expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeDisabled();expect(compatibleBridge.findCompatibleTracks).not.toHaveBeenCalled();fireEvent.change(screen.getByLabelText("BPM tolerance"),{target:{value:"4"}});fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));await screen.findByRole("alert");expect(screen.getByRole("alert")).toHaveTextContent("Source BPM is unknown");
 });
 it("invalidates a preview when library measurements change",async()=>{
 const {bridge}=setup();await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));await screen.findByText("Compatible key: 8A → 9B (fuzzy)");
 // Subscription is established on mount; capture the real registered handler.
 const handler=vi.mocked(bridge.onLibraryChange).mock.calls[0]?.[0];expect(handler).toBeDefined();handler!({track_ids:["match"]});await waitFor(()=>expect(screen.queryByText("Compatible key: 8A → 9B (fuzzy)")).not.toBeInTheDocument());
 });
 it("does not save a preview after the library revision has changed",async()=>{
 const {bridge}=setup();bridge.getState=vi.fn(async()=>({...(await fakeBridge([]).getState()),revision:99}));await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));await screen.findByText("Compatible key: 8A → 9B (fuzzy)");fireEvent.click(screen.getByLabelText("Select Track match"));fireEvent.click(screen.getByRole("button",{name:"Save selected to playlist"}));await screen.findByRole("alert");expect(screen.getByRole("alert")).toHaveTextContent("Library changed");expect(bridge.editPlaylist).not.toHaveBeenCalled();
 });
 it("requires successful library options loading and offers a retry",async()=>{
 setup(undefined,bridge=>{let attempt=0;bridge.getPlaylistPreferences=vi.fn(async()=>{if(attempt++===0)throw Error("Library options unavailable");return {revision:0,preferences:{time_zone:null,key_mixing_mode:"fuzzy" as const}};});});await screen.findByText("Library options unavailable");expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeDisabled();fireEvent.click(screen.getByRole("button",{name:"Retry library options"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());expect(screen.getByLabelText("Key mode")).toHaveValue("fuzzy");
 });

 it("shows authoritative stale-save rejection after the client precheck without issuing generic edits",async()=>{
 const {bridge,compatibleBridge}=setup();vi.mocked(compatibleBridge.saveCompatiblePlaylist).mockRejectedValue(Error("Library changed; refresh matches before saving."));await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));await screen.findByText("Compatible key: 8A → 9B (fuzzy)");fireEvent.click(screen.getByLabelText("Select Track match"));fireEvent.click(screen.getByRole("button",{name:"Save selected to playlist"}));await screen.findByRole("alert");expect(screen.getByRole("alert")).toHaveTextContent("Library changed");expect(bridge.editPlaylist).not.toHaveBeenCalled();expect(screen.queryByText(/Saved 1 track/)).not.toBeInTheDocument();
 });

 it("delegates preview to the persistent player without a second selection or player load",async()=>{
 const onPreview=vi.fn(async()=>{});const {bridge,onSelect}=setup(undefined,undefined,onPreview);await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));await screen.findByText("Compatible key: 8A → 9B (fuzzy)");fireEvent.click(screen.getByRole("button",{name:"Preview Track match"}));await waitFor(()=>expect(onPreview).toHaveBeenCalledWith(expect.objectContaining({id:"match"})));expect(onSelect).not.toHaveBeenCalled();expect(bridge.playerLoad).not.toHaveBeenCalled();expect(bridge.playerPlay).not.toHaveBeenCalled();
 });

});
