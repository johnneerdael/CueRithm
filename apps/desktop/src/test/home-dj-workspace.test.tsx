import {act,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import type {CompatibleBridge} from "../compatible-types";
import type {TracklistBridge} from "../tracklist-types";
import {fakeBridge,track} from "./bridge";
import type {LibraryBackupBridge} from "../backup-types";
import type {ManualPublicationApi} from "../manual-publication-types";
import type {Analysis,Detail,Publication} from "../types";
import type {PlaylistChange} from "../playlist-types";

function backupBridge(blocked=false):LibraryBackupBridge {
 const pending={id:"restore",created:1,database_sha256:"hash",blocked_reason:"Music changed after scheduling"};
 return {pending:vi.fn(async()=>blocked?pending:null),cancel:vi.fn(async()=>{}),retry:vi.fn(async()=>({...pending,blocked_reason:null})),chooseFolder:vi.fn(async()=>null),create:vi.fn(),inspect:vi.fn(),stage:vi.fn()};
}

describe("Home DJ workspace integration",()=>{
 it("opens compatible tracks and auditions once through the persistent player",async()=>{
  const bridge=fakeBridge([track("source"),track("match")]);
  const compatibleBridge:CompatibleBridge={findCompatibleTracks:vi.fn(async()=>({source:track("source"),key_mode:"harmonic" as const,matches:[{track:track("match"),reasons:["Matching BPM"],bpm_delta:0,bpm_multiplier:1,energy_delta:null,same_key:true}],checked:1,excluded_unknown:0,library_revision:1})),saveCompatiblePlaylist:vi.fn(async()=>({edit_id:"saved",revision:1,created_ids:["new-set"],affected_ids:["new-set"]}))};
  render(<App bridge={bridge} compatibleBridge={compatibleBridge}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Compatible tracks"}));
  await screen.findByRole("heading",{name:"Compatible tracks",level:1});
  await screen.findByRole("option",{name:"Artist — Track source"});
  fireEvent.change(screen.getByRole("combobox",{name:"Source track"}),{target:{value:"source"}});
  await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));
  fireEvent.click(await screen.findByRole("button",{name:"Preview Track match"}));
  await waitFor(()=>expect(bridge.playerPlay).toHaveBeenCalledTimes(1));
  expect(bridge.playerLoad).toHaveBeenCalledTimes(1);
  expect(within(screen.getByRole("region",{name:"Music player"})).getByText("Track match")).toBeInTheDocument();
 });

 it("opens the tracklist matcher and keeps select-all scoped away from hidden library tracks",async()=>{
  const bridge=fakeBridge([track("one")]);
  const tracklistBridge:TracklistBridge={matchTracklist:vi.fn(async()=>({rows:[],library_revision:1,playlist_revision:0,candidate_limit:10})),saveTracklistPlaylist:vi.fn(async()=>({edit_id:"e",revision:1,created_ids:["set"],affected_ids:["set"]}))};
  render(<App bridge={bridge} tracklistBridge={tracklistBridge}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Tracklist matcher"}));
  await screen.findByRole("heading",{name:"Match a tracklist"});
  fireEvent.keyDown(window,{key:"a",ctrlKey:true});
  expect(bridge.selectTrackIds).not.toHaveBeenCalled();
  fireEvent.change(screen.getByRole("textbox",{name:"Reference tracklist"}),{target:{value:"Artist - One"}});
  fireEvent.click(screen.getByRole("button",{name:"Find library candidates"}));
  await screen.findByRole("region",{name:"Review tracklist matches"});
  expect(tracklistBridge.matchTracklist).toHaveBeenCalledWith("Artist - One",expect.objectContaining({format:"text",mode:"exact"}));
 });

 it("does not let an old save navigate away from a reopened compatible workspace",async()=>{
  const bridge=fakeBridge([track("source"),track("match")]);let saved=false;
  bridge.getPlaylists=vi.fn(async()=>({tree:{revision:saved?1:0,nodes:saved?[{id:"saved",name:"Saved practice",kind:"playlist" as const,parent_id:null,position:0,favorite_position:null,definition:null,deleted:false,deleted_by:null}]:[]},statuses:[],latest_undo:null,service_error:null}));
  let finish!:(change:PlaylistChange)=>void;
  const compatibleBridge:CompatibleBridge={findCompatibleTracks:vi.fn(async()=>({source:track("source"),key_mode:"harmonic" as const,matches:[{track:track("match"),reasons:["Matching BPM"],bpm_delta:0,bpm_multiplier:1,energy_delta:null,same_key:true}],checked:1,excluded_unknown:0,library_revision:1})),saveCompatiblePlaylist:vi.fn(()=>new Promise<PlaylistChange>(resolve=>{finish=resolve;}))};
  render(<App bridge={bridge} compatibleBridge={compatibleBridge}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Compatible tracks"}));
  await screen.findByRole("option",{name:"Artist — Track source"});fireEvent.change(screen.getByLabelText("Source track"),{target:{value:"source"}});
  await waitFor(()=>expect(screen.getByRole("button",{name:"Find compatible tracks"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Find compatible tracks"}));
  fireEvent.click(await screen.findByLabelText("Select Track match"));fireEvent.click(screen.getByRole("button",{name:"Save selected to playlist"}));
  await waitFor(()=>expect(compatibleBridge.saveCompatiblePlaylist).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("button",{name:/^Tracks/}));fireEvent.click(screen.getByRole("button",{name:"Compatible tracks"}));
  await screen.findByRole("heading",{name:"Compatible tracks",level:1});
  await act(async()=>{saved=true;finish({edit_id:"edit",revision:1,created_ids:["saved"],affected_ids:["saved"]});});
  await screen.findByRole("button",{name:"Playlist Saved practice"});
  expect(screen.getByRole("heading",{name:"Compatible tracks",level:1})).toBeInTheDocument();
 });

 it("shows ordinary playlist flow in canonical membership order independently of table rows",async()=>{
  const bridge=fakeBridge([track("two"),track("one")]);
  bridge.getPlaylists=vi.fn(async()=>({tree:{revision:1,nodes:[{id:"set",name:"Practice",kind:"playlist" as const,parent_id:null,position:0,favorite_position:null,definition:null,deleted:false,deleted_by:null}]},statuses:[],latest_undo:null,service_error:null}));
  bridge.playlistMembers=vi.fn(async()=>["one","two"]);
  render(<App bridge={bridge}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Playlist Practice"}));
  fireEvent.click(await screen.findByRole("button",{name:"Playlist flow"}));
  const flow=await screen.findByRole("region",{name:"Playlist flow"});
  const choices=within(flow).getAllByRole("button",{name:/^Select Track/});
  expect(choices.map(button=>button.getAttribute("aria-label"))).toEqual(["Select Track one","Select Track two"]);
  expect(bridge.playlistMembers).toHaveBeenCalledWith("set",true);
  fireEvent.click(choices[0]);
  expect(choices[0]).toHaveAttribute("aria-pressed","true");
 });

 it("opens library backup from the sidebar without scheduling a restore",async()=>{
  const backup=backupBridge();render(<App bridge={fakeBridge([])} backupBridge={backup}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Library backup"}));
  await screen.findByRole("region",{name:"Library backup and restore"});
  expect(backup.stage).not.toHaveBeenCalled();expect(backup.create).not.toHaveBeenCalled();
 });

 it("surfaces a blocked startup restore in the normal backup view with cancellation",async()=>{
  const backup=backupBridge(true);render(<App bridge={fakeBridge([])} backupBridge={backup}/>);
  await screen.findByText("Music changed after scheduling");
  fireEvent.click(screen.getByRole("button",{name:"Cancel scheduled restore"}));
  await waitFor(()=>expect(backup.cancel).toHaveBeenCalledTimes(1));
 });

 it("refreshes saved publication and bar-jump timing without reloading the audio",async()=>{
  const bridge=fakeBridge([track("source")]);let corrected=false;
  const original:Publication={grid:{bpm:128,startTime:0},cues:[{name:"Start",startTime:0,type:"1",position:0,color:"yellow"}],implementation_sha256:"measured",inputs_sha256:"original"};
  const publication=()=>({...original,grid:{bpm:corrected?130:128,startTime:0}});
  const analysis=():Analysis=>({publication:publication(),analysis:{duration_seconds:240,key:{camelot:"8A"},tempo:{bpm:128,beats_seconds:[0,0.46875],downbeats_seconds:[0]},energy:{level:6,method:"authored"},lexicon_cues:{cues:[],emergency_loop:null}},source_sample_rate:44100,complete:true});
  const detail=():Detail=>({track:track("source"),analysis:analysis(),source_sha256:"hash-source",pcm_sha256:"pcm"});
  const load=bridge.playerLoad;bridge.playerLoad=vi.fn(async id=>{await load(id);return detail();});bridge.getTrack=vi.fn(async()=>detail());
  const guard={track_id:"source",source_sha256:"hash-source",entity_epoch:1,cue_revision_id:"cue"};
  const api:ManualPublicationApi={snapshot:vi.fn(async()=>({guard,library_revision:1,publication:publication(),duration_seconds:240})),templates:vi.fn(async()=>[]),history:vi.fn(async()=>[]),preview:vi.fn(async request=>({id:"preview",library_revision:1,changes:[{track_id:"source",before:original,after:{...original,grid:request.grid,cues:request.cues},preserved_manual:false}]})),apply:vi.fn(async()=>{corrected=true;return {id:"edit",track_ids:["source"],library_revision:1};}),undo:vi.fn(),saveTemplate:vi.fn(),deleteTemplate:vi.fn(),previewTemplate:vi.fn()};
  render(<App bridge={bridge} publicationApi={api}/>);
  await screen.findByText("Track source");fireEvent.doubleClick(screen.getByText("Track source"));
  fireEvent.click(await screen.findByRole("button",{name:"Edit cues & grid"}));
  fireEvent.change(await screen.findByLabelText("Grid BPM"),{target:{value:"130"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview corrections"}));
  fireEvent.click(await screen.findByRole("button",{name:"Save preview"}));
  await waitFor(()=>expect(within(screen.getByRole("region",{name:"Music player"})).getByText("130.00")).toBeInTheDocument());
  expect(bridge.playerLoad).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(window,{key:"ArrowRight"});
  await waitFor(()=>expect(bridge.playerSeek).toHaveBeenCalledWith(240/130));
 });
});
