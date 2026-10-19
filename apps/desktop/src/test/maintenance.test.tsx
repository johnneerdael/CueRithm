import {render,screen,fireEvent,waitFor,within,act} from "@testing-library/react";
import {it,expect,vi,beforeEach} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import {fakeMaintenance,stamp,review,group,duplicateScan,candidate} from "./maintenance-bridge";

let uuid=0;
beforeEach(()=>{uuid=0;vi.stubGlobal("crypto",{...globalThis.crypto,randomUUID:()=>"op-"+ ++uuid});});

async function open(maintenance:ReturnType<typeof fakeMaintenance>,library=fakeBridge([track("a")])){
  render(<App bridge={library} maintenance={maintenance}/>);
  await screen.findByText("Track a");
  fireEvent.click(screen.getByRole("button",{name:"Maintenance"}));
  await screen.findByRole("heading",{name:"Maintenance"});
  return library;
}
const tab=(name:string)=>fireEvent.click(screen.getByRole("tab",{name}));

it("reports missing, unavailable and undecodable files separately and flags a stale report",async()=>{
  const maintenance=fakeMaintenance();
  maintenance.scanBroken.mockResolvedValue({id:"broken",created:1,stamp,stale:true,stale_reason:"Library changed",request:{inspect_audio:true},decoder_identity:"decoder-1",tracks:[
    {id:"1",display_path:"/music/gone.mp3",state:"missing",detail:null,observed_sha256:null,source_changed:false},
    {id:"2",display_path:"/Volumes/Away/x.flac",state:"unavailable",detail:null,observed_sha256:null,source_changed:false},
    {id:"3",display_path:"/music/bad.wav",state:"decode_error",detail:"bad header",observed_sha256:null,source_changed:false},
    {id:"4",display_path:"/music/fine.wav",state:"present",detail:null,observed_sha256:null,source_changed:false}]});
  await open(maintenance);tab("Missing files");
  fireEvent.click(screen.getByLabelText(/Decode each file/));
  fireEvent.click(screen.getByRole("button",{name:"Scan library"}));
  await screen.findByText(/3 problems/);
  expect(maintenance.scanBroken).toHaveBeenCalledWith("op-1",{inspect_audio:true});
  expect(screen.getByRole("alert")).toHaveTextContent("out of date");
  const table=within(screen.getByRole("table"));
  expect(table.getByText("Missing")).toBeInTheDocument();expect(table.getByText("Unavailable")).toBeInTheDocument();expect(table.getByText("Decode error")).toBeInTheDocument();
  expect(table.queryByText("/music/fine.wav")).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"All tracks"}));
  expect(table.getByText("/music/fine.wav")).toBeInTheDocument();
});

it("keeps the operation slot until a cancelled scan settles",async()=>{
  const maintenance=fakeMaintenance();let finish!:(v:never)=>void;
  maintenance.scanBroken.mockImplementation(()=>new Promise((_,reject)=>{finish=reject as never;}));
  await open(maintenance);tab("Missing files");
  fireEvent.click(screen.getByRole("button",{name:"Scan library"}));
  const status=await screen.findByRole("status");
  act(()=>maintenance.emitProgress({operation_id:"op-1",kind:"broken",progress:{completed:5,total:20,current_path:"/music/x.mp3"}}));
  expect(status).toHaveTextContent("5 of 20");
  act(()=>maintenance.emitProgress({operation_id:"other",kind:"broken",progress:{completed:9,total:20,current_path:null}}));
  expect(status).toHaveTextContent("5 of 20");
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
  expect(maintenance.cancel).toHaveBeenCalledWith("op-1");
  expect(screen.getByRole("button",{name:"Scan library"})).toBeDisabled();
  await act(async()=>finish({code:"cancelled",message:"cancelled"} as never));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Scan library"})).toBeEnabled());
  expect(screen.queryByRole("alert")).toBeNull();
});

const unused=(extra={})=>({id:"unused",created:1,stamp,stale:false,stale_reason:null,request:{root:"/music",extension_mode:"all" as const,extensions:[]},size_bytes:3000,complete:true,issues:[],
  files:[{id:"f1",display_path:"/music/old.mp3",size_bytes:1000,source_sha256:"h1"},{id:"f2",display_path:"/music/old.jpg",size_bytes:2000,source_sha256:"h2"}],...extra});

it("previews every file and requires explicit confirmation before removing unused files",async()=>{
  const maintenance=fakeMaintenance(),library=fakeBridge([track("a")]);
  vi.mocked(library.chooseFolder).mockResolvedValue("/music");
  maintenance.scanUnused.mockResolvedValue(unused());
  maintenance.previewUnusedRemoval.mockResolvedValue({id:"rp",scan_id:"unused",stamp,size_bytes:1500,directories:["/music/empty"],files:[
    {id:"f1",display_path:"/music/old.mp3",size_bytes:1000,source_sha256:"h1",role:"selected"},{id:"s1",display_path:"/music/old.asd",size_bytes:500,source_sha256:"h3",role:"sibling"},
    {id:"m1",display_path:"/music/.DS_Store",size_bytes:0,source_sha256:"h4",role:"folder_metadata"}]});
  await open(maintenance,library);tab("Unused files");
  fireEvent.click(screen.getByRole("button",{name:"Choose folder…"}));await screen.findByText("/music");
  fireEvent.click(screen.getByRole("button",{name:"Scan folder"}));
  await screen.findByText(/2 unused files/);
  expect(screen.getByRole("button",{name:/Review removal/})).toBeDisabled();
  fireEvent.click(screen.getByLabelText("Select /music/old.mp3"));
  fireEvent.click(screen.getByRole("button",{name:/Review removal/}));
  const preview=await screen.findByRole("region",{name:"Removal preview"});
  expect(within(preview).getByText("/music/old.asd")).toBeInTheDocument();expect(within(preview).getByText("/music/.DS_Store")).toBeInTheDocument();expect(within(preview).getByText("/music/empty")).toBeInTheDocument();
  expect(maintenance.applyUnusedRemoval).not.toHaveBeenCalled();
  fireEvent.click(within(preview).getByRole("button",{name:/^Remove 3 files/}));
  const dialog=await screen.findByRole("alertdialog");
  expect(maintenance.applyUnusedRemoval).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button",{name:"Remove 3 files"}));
  await waitFor(()=>expect(maintenance.applyUnusedRemoval).toHaveBeenCalledWith("op-3","rp"));
  await screen.findByText(/Originals are kept for Restore/);
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

it("does not allow removal from a partial or stale unused-file scan",async()=>{
  const maintenance=fakeMaintenance(),library=fakeBridge([track("a")]);
  vi.mocked(library.chooseFolder).mockResolvedValue("/music");
  maintenance.scanUnused.mockResolvedValue(unused({complete:false,issues:[{display_path:"/music/locked",message:"permission denied"}]}));
  await open(maintenance,library);tab("Unused files");
  fireEvent.click(screen.getByRole("button",{name:"Choose folder…"}));await screen.findByText("/music");
  fireEvent.click(screen.getByRole("button",{name:"Scan folder"}));
  await screen.findByText(/This scan is partial/);
  expect(screen.getByLabelText("Select /music/old.mp3")).toBeDisabled();
  expect(screen.getByRole("button",{name:/Review removal/})).toBeDisabled();
});

it("restores a removal and offers recovery retry for interrupted removals",async()=>{
  const maintenance=fakeMaintenance();
  const result=(id:string,extra={})=>({directories:[],id,preview_id:"p",stamp,files:[{id:"f",display_path:"/m/a.mp3",size_bytes:10,source_sha256:"h",role:"selected" as const}],size_bytes:10,restored:false,undo_of:null,...extra});
  maintenance.unusedRemovalHistory.mockResolvedValue([
    {restore_available:true,result:result("r1"),state:"committed",error:null,undone_by:null,cleanup_pending:false},
    {restore_available:false,result:result("r2"),state:"conflict",error:"A newer file exists",undone_by:null,cleanup_pending:true}]);
  maintenance.retryUnusedRecovery.mockResolvedValue([
    {restore_available:true,result:result("r1"),state:"committed",error:null,undone_by:null,cleanup_pending:false},
    {restore_available:true,result:result("r2"),state:"committed",error:null,undone_by:null,cleanup_pending:false}]);
  await open(maintenance);tab("Unused files");
  expect(await screen.findByText("A newer file exists")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:/Retry recovery/}));
  await waitFor(()=>expect(maintenance.retryUnusedRecovery).toHaveBeenCalledWith("op-1","r2"));
  const restores=await screen.findAllByRole("button",{name:/^Restore$/});
  expect(restores).toHaveLength(2);
  fireEvent.click(restores[0]);
  await waitFor(()=>expect(maintenance.restoreUnusedRemoval).toHaveBeenCalledWith("op-2","r1"));
});

it("requires an irreversible-disposal acknowledgement and shows the Restore loss",async()=>{
  const maintenance=fakeMaintenance();
  maintenance.unusedRemovalHistory.mockResolvedValue([{restore_available:true,state:"committed",error:null,undone_by:null,cleanup_pending:false,
    result:{directories:[],id:"r1",preview_id:"p",stamp,files:[{id:"f",display_path:"/m/a.mp3",size_bytes:2048,source_sha256:"h",role:"selected"}],size_bytes:2048,restored:false,undo_of:null}}]);
  maintenance.previewBackupDisposal.mockResolvedValue({id:"dp",operation_ids:["r1"],size_bytes:2048,restores_lost:["r1"],files:[{operation_id:"r1",file_id:"f",display_source_path:"/m/a.mp3",size_bytes:2048,source_sha256:"h"}]});
  await open(maintenance);tab("Retained backups");
  fireEvent.click(await screen.findByLabelText(/Select removal r1/));
  fireEvent.click(screen.getByRole("button",{name:/Review disposal of backups/}));
  const preview=await screen.findByRole("region",{name:"Disposal preview"});
  expect(preview).toHaveTextContent("Restore will be lost for 1 removal");expect(preview).toHaveTextContent("2.0 KB");
  fireEvent.click(within(preview).getByRole("button",{name:/Dispose of backups/}));
  const dialog=await screen.findByRole("alertdialog");
  const confirm=within(dialog).getByRole("button",{name:"Dispose of backups"});
  expect(confirm).toBeDisabled();
  fireEvent.click(within(dialog).getByRole("checkbox"));
  expect(confirm).toBeEnabled();
  fireEvent.click(confirm);
  await waitFor(()=>expect(maintenance.disposeBackups).toHaveBeenCalledWith("op-2","dp"));
});

it("walks a duplicate group from scan through review, preview and a confirmed merge",async()=>{
  const maintenance=fakeMaintenance(),library=fakeBridge([track("a")]);
  maintenance.beginReview.mockResolvedValue(review([group("g1",["a","b"])],4));
  maintenance.editReview.mockResolvedValue(review([group("g1",["a","b"],{keeper:"b"})],5));
  maintenance.previewMerge.mockResolvedValue({id:"mp",review_id:"review",review_revision:5,created:1,stamp,playlists:[{id:"pl",before:["a","b"],after:["b"]}],plan:{groups:[{group_id:"g1",mode:"unlocked",exact_audio:true,keeper:"b",archive_ids:["a"],unarchive_keeper:false,copies:[{from:"a",to:"b"}]}]}});
  await open(maintenance,library);
  fireEvent.click(screen.getByRole("button",{name:"Scan for duplicates"}));
  fireEvent.click(await screen.findByRole("button",{name:/Review 1 group/}));
  await screen.findByRole("article",{name:/Duplicate group/});
  fireEvent.click(screen.getByLabelText("Keep Artist – Title b"));
  await waitFor(()=>expect(maintenance.editReview).toHaveBeenCalledWith("op-3","review",4,{kind:"group",group_id:"g1",edit:{kind:"select",track_id:"b"}}));
  fireEvent.click(await screen.findByLabelText(/Include group/));
  fireEvent.click(screen.getByRole("button",{name:/Preview merge/}));
  const preview=await screen.findByRole("region",{name:"Merge preview"});
  expect(maintenance.previewMerge).toHaveBeenCalledWith(expect.any(String),"review",5,["g1"]);
  expect(preview).toHaveTextContent("1 tracks will be archived");expect(preview).toHaveTextContent("2 → 1 tracks");
  fireEvent.click(within(preview).getByRole("button",{name:/Apply merge/}));
  expect(maintenance.applyMerge).not.toHaveBeenCalled();
  fireEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button",{name:"Apply merge"}));
  await waitFor(()=>expect(maintenance.applyMerge).toHaveBeenCalledWith(expect.any(String),"mp"));
  await screen.findByText(/No music files were deleted/);
});

it("reloads the review after a revision conflict and surfaces the error",async()=>{
  const maintenance=fakeMaintenance();
  maintenance.beginReview.mockResolvedValue(review([group()],1));
  maintenance.editReview.mockRejectedValue({code:"operation_failed",message:"Review revision changed"});
  maintenance.getReview.mockResolvedValue(review([group("g1",["a","b"],{keeper:"b"})],2));
  await open(maintenance);
  fireEvent.click(screen.getByRole("button",{name:"Scan for duplicates"}));
  fireEvent.click(await screen.findByRole("button",{name:/Review 1 group/}));
  await screen.findByRole("article",{name:/Duplicate group/});
  fireEvent.click(screen.getByLabelText("Keep Artist – Title b"));
  expect(await screen.findByRole("alert")).toHaveTextContent("Review revision changed");
  expect(maintenance.getReview).toHaveBeenCalled();
  expect(screen.getByLabelText("Keep Artist – Title b")).toBeChecked();
});

it("shows merge history with Undo and interrupted merges needing recovery",async()=>{
  const maintenance=fakeMaintenance();
  const done={id:"m1",preview_id:"p",stamp,archived_ids:["x","y"],restored_ids:[],affected_playlists:[],undo_of:null,updated_ids:["z"]};
  maintenance.mergeHistory.mockResolvedValue([{result:done,created:1000,undone_by:null}]);
  maintenance.recovery.mockResolvedValue([{id:"j1",kind:"merge",state:"conflict",created:1000,error:"File edited since merge"}]);
  await open(maintenance);
  expect(await screen.findByText("File edited since merge")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:/Retry recovery/}));
  await waitFor(()=>expect(maintenance.retryRecovery).toHaveBeenCalledWith("op-1","j1"));
  fireEvent.click(await screen.findByRole("button",{name:/^Undo$/}));
  await waitFor(()=>expect(maintenance.undoMerge).toHaveBeenCalledWith("op-2","m1"));
});

it("keeps ignored tracks restorable through the stamped ignore list",async()=>{
  const maintenance=fakeMaintenance();
  maintenance.ignores.mockResolvedValue({stamp,tracks:[{id:"t9",display_path:"/music/t9.mp3",title:"Nine",artist:"Band"}]});
  await open(maintenance);
  fireEvent.click(await screen.findByRole("button",{name:/Stop ignoring/}));
  await waitFor(()=>expect(maintenance.setIgnores).toHaveBeenCalledWith(["t9"],false,stamp));
});

it("rechecks only the visible tab when another view changes the library",async()=>{
  const maintenance=fakeMaintenance();
  maintenance.brokenScan.mockResolvedValue({id:"broken",created:1,stamp,stale:true,stale_reason:"Tracks changed",request:{inspect_audio:false},decoder_identity:null,tracks:[]});
  await open(maintenance);tab("Missing files");
  fireEvent.click(screen.getByRole("button",{name:"Scan library"}));
  await screen.findByText(/0 tracks checked/);
  act(()=>maintenance.emitChange());
  await waitFor(()=>expect(maintenance.brokenScan).toHaveBeenCalledWith(expect.any(String),"broken"));
  expect(await screen.findByRole("alert")).toHaveTextContent("Tracks changed");
});

it("archives only definite broken tracks after confirmation and never offers unavailable ones",async()=>{
  const maintenance=fakeMaintenance(),library=fakeBridge([track("a")]);
  maintenance.scanBroken.mockResolvedValue({id:"broken",created:1,stamp,stale:false,stale_reason:null,request:{inspect_audio:false},decoder_identity:null,tracks:[
    {id:"1",display_path:"/music/gone.mp3",state:"missing",detail:null,observed_sha256:null,source_changed:false},
    {id:"2",display_path:"/Volumes/Away/x.flac",state:"unavailable",detail:null,observed_sha256:null,source_changed:false}]});
  await open(maintenance,library);tab("Missing files");
  fireEvent.click(screen.getByRole("button",{name:"Scan library"}));
  await screen.findByText(/2 problems/);
  expect(screen.getByLabelText("Select /Volumes/Away/x.flac")).toBeDisabled();
  expect(screen.getByRole("button",{name:/Archive\s*selected/})).toBeDisabled();
  fireEvent.click(screen.getByLabelText("Select /music/gone.mp3"));
  fireEvent.click(screen.getByRole("button",{name:/Archive 1 selected/}));
  const dialog=await screen.findByRole("alertdialog");
  expect(library.archiveTrack).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button",{name:"Archive 1 track"}));
  await waitFor(()=>expect(library.archiveTrack).toHaveBeenCalledWith("1",true));
  await screen.findByText(/Archived 1 track/);
});

it("states the outcome of each duplicate group in plain language",async()=>{
  const maintenance=fakeMaintenance();
  maintenance.beginReview.mockResolvedValue(review([group("g1",["a","b","c"],{keeper:"a",tags_from:"b",ignored:["c"]}),group("g2",["d","e"],{state:"locked"}),group("g3",["f","g"],{state:"copy_cues",tags_from:"f",keeper:null})]));
  await open(maintenance);
  fireEvent.click(screen.getByRole("button",{name:"Scan for duplicates"}));
  fireEvent.click(await screen.findByRole("button",{name:/Review 1 group/}));
  await screen.findAllByRole("article",{name:/Duplicate group/});
  expect(screen.getByText("Track #1, #3 will not be archived. Tags and cues of track #2 will be used. 1 track will be archived.")).toBeInTheDocument();
  expect(screen.getByText("Locked. No tracks will be archived.")).toBeInTheDocument();
  expect(screen.getByText("Tags and cues of track #1 will be copied to the other track. No tracks will be archived.")).toBeInTheDocument();
});

it("reviews retained tagging and cleanup originals, and says when Undo would be lost",async()=>{
  const maintenance=fakeMaintenance(),library=fakeBridge([track("a")]);
  vi.mocked(library.getTaggingStorage).mockResolvedValue({original_files:3,original_bytes:3*1048576,pending_cleanup:0});
  maintenance.previewRetainedOriginals.mockResolvedValue({id:"rp",include_undoable:true,size_bytes:3145728,undo_lost_writes:2,files:[
    {source:"tagging",write_id:"w1",display_path:"/music/a.flac",size_bytes:1048576,undo_lost:false},
    {source:"cleanup",write_id:"w2",display_path:"/music/b.flac",size_bytes:1048576,undo_lost:true},
    {source:"cleanup",write_id:"w3",display_path:"/music/c.flac",size_bytes:1048576,undo_lost:true}]});
  await open(maintenance,library);tab("Retained backups");
  expect(await screen.findByText(/3 originals · 3.0 MB/)).toBeInTheDocument();
  fireEvent.click(screen.getByLabelText(/Also include originals that let me Undo/));
  fireEvent.click(screen.getByRole("button",{name:/Review disposal of originals/}));
  const preview=await screen.findByRole("region",{name:"Originals disposal preview"});
  expect(maintenance.previewRetainedOriginals).toHaveBeenCalledWith(expect.any(String),true);
  expect(preview).toHaveTextContent("Undo will stop working for 2 cleanup writes");
  expect(within(preview).getByText("/music/b.flac").closest("li")).toHaveTextContent("backs Undo");
  fireEvent.click(within(preview).getByRole("button",{name:/Dispose of originals/}));
  const dialog=await screen.findByRole("alertdialog");
  const confirm=within(dialog).getByRole("button",{name:"Dispose of originals"});
  expect(confirm).toBeDisabled();
  expect(dialog).toHaveTextContent("cleanup Undo will stop working");
  fireEvent.click(within(dialog).getByRole("checkbox"));
  fireEvent.click(confirm);
  await waitFor(()=>expect(maintenance.disposeRetainedOriginals).toHaveBeenCalledWith(expect.any(String),"rp"));
});

it("does not offer retained-original disposal when none are kept, and finishes an unfinished one",async()=>{
  const maintenance=fakeMaintenance();
  maintenance.retainedDisposalHistory.mockResolvedValue([{id:"d1",preview_id:"p",file_count:2,size_bytes:2048,undo_lost_writes:0,created:1000,state:"committed",cleanup_pending:true,error:"Deleting originals is unfinished: permission denied"}]);
  const finished=[{id:"d1",preview_id:"p",file_count:2,size_bytes:2048,undo_lost_writes:0,created:1000,state:"committed",cleanup_pending:false,error:null}];
  maintenance.retryRetainedDisposal.mockImplementation(async()=>{maintenance.retainedDisposalHistory.mockResolvedValue(finished);return finished;});
  await open(maintenance);tab("Retained backups");
  const section=within(await screen.findByRole("region",{name:"Preparation and cleanup originals"}));
  expect(section.getByRole("button",{name:/Review disposal of originals/})).toBeDisabled();
  expect(await section.findByText(/permission denied/)).toBeInTheDocument();
  fireEvent.click(section.getByRole("button",{name:/Retry/}));
  await waitFor(()=>expect(maintenance.retryRetainedDisposal).toHaveBeenCalledWith(expect.any(String),"d1"));
  await waitFor(()=>expect(section.queryByText(/permission denied/)).toBeNull());
});
