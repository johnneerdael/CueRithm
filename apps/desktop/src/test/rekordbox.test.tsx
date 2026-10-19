import {render,screen,fireEvent,waitFor,within} from "@testing-library/react";
import {it,expect,vi,beforeEach} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import {fakeMaintenance} from "./maintenance-bridge";
import type {RekordboxBridge} from "../rekordbox-api";
import type {NativeSyncPrepared,NativeSyncRun,RekordboxPreflight} from "../rekordbox-types";

let uuid=0;
beforeEach(()=>{uuid=0;vi.stubGlobal("crypto",{...globalThis.crypto,randomUUID:()=>"op-"+ ++uuid});});

const preflight=(extra:Partial<RekordboxPreflight>={}):RekordboxPreflight=>({platform_supported:true,installations:[{version:"7.2.19.0342",major:7}],database_path:"/Users/dj/Library/Pioneer/rekordbox/master.db",database_state:"present",key_ready:true,key_error:null,schema_fingerprint:"6bd02ac0a3f9",write_supported:true,write_profiles:["rekordbox 7.2.19.0342"],targets:[{id:"t1",identity:{database_id:"d",device_id:null,database_version:"7",local_update_count:10},schema_fingerprint:"x",revision:1,records:5,tracks:3,issues:[]}],...extra});
const diff=(field:string,decision:any,local:unknown,remote:unknown)=>({field,decision,local,remote,baseline_local:null,baseline_remote:null,owner:"local" as const,acknowledged:true});
const prepared=(over:Partial<NativeSyncPrepared>={}):NativeSyncPrepared=>({target_id:"t1",snapshot_usn:10,
  preview:{target_id:"t1",revision:2,writes_enabled:false,write_blocker:"",summary:{},tracks:[
    {content_id:"101",local_track_id:"l1",status:"linked",expected_fingerprint:"fp",blocked:null,fields:[diff("title","push_local","New title","Old title"),diff("genre","pull_remote","House","Tech House"),diff("key","empty_local",null,"8A")]},
    {content_id:"102",local_track_id:"l2",status:"identity_changed",expected_fingerprint:null,blocked:"identity_changed",fields:[]}]},
  plan:{tracks:[{content_id:"101",local_track_id:"l1",fields:["title"]}],skipped:[{content_id:"101",field:"key",decision:"empty_local"}],blocked:[["102","identity_changed"]],pull_available:1},
  playlists:{items:[{path:["Sets","Friday"],kind:"playlist",action:"create",tracks:12,unlinked:2,materialized:false},{path:["Warmup"],kind:"playlist",action:"skip_existing",tracks:5,unlinked:0,materialized:false},{path:["Old"],kind:"playlist",action:"unchanged",tracks:1,unlinked:0,materialized:false},{path:["Peak"],kind:"smartlist",action:"create",tracks:0,unlinked:0,materialized:false},{path:["Regex"],kind:"playlist",action:"create",tracks:4,unlinked:0,materialized:true}]},...over});
const run:NativeSyncRun={id:"r1",created:1700000000000,target_id:"t1",state:"committed",usn_before:10,usn_after:11,tracks:[{content_id:"101",local_track_id:"l1",fields:["title"]}],playlists:{items:[]},skipped:1,backup:"/data/backups/r1/master.backup.db",journal:"/data/backups/r1/journal.json"};
function fakeRekordbox(){
  return {
    preflight:vi.fn<RekordboxBridge["preflight"]>(async()=>preflight()),
    playlists:vi.fn<RekordboxBridge["playlists"]>(async()=>[{id:"p1",parent_id:"root",name:"Warmup",is_folder:false,tracks:12},{id:"f1",parent_id:"root",name:"Sets",is_folder:true,tracks:0}]),
    importTracks:vi.fn<RekordboxBridge["importTracks"]>(async()=>({target_id:"t1",revision:1,changed:true,tracks_created:2,jobs_created:2,statuses:{linked:3}})),
    prepare:vi.fn<RekordboxBridge["prepare"]>(async()=>prepared()),
    apply:vi.fn<RekordboxBridge["apply"]>(async()=>({run,conflicts:[]})),
    history:vi.fn<RekordboxBridge["history"]>(async()=>[]),
  } satisfies RekordboxBridge;
}
async function open(rekordbox:ReturnType<typeof fakeRekordbox>){
  const library=fakeBridge([track("a")]),maintenance=fakeMaintenance();
  render(<App bridge={library} maintenance={maintenance} rekordbox={rekordbox}/>);
  await screen.findByText("Track a");
  fireEvent.click(screen.getByRole("button",{name:"Rekordbox"}));
  await screen.findByRole("heading",{name:"rekordbox"});
  return {library,maintenance};
}

it("reports what is ready before anything can be done",async()=>{
  const rekordbox=fakeRekordbox();
  rekordbox.preflight.mockResolvedValue(preflight({key_ready:false,key_error:"rekordbox agent options were not found",write_supported:false,schema_fingerprint:null}));
  await open(rekordbox);
  const checks=within(await screen.findByRole("list",{name:"Preflight checks"}));
  expect(checks.getByText("Version 7.2.19.0342")).toBeInTheDocument();
  expect(checks.getByText("rekordbox agent options were not found")).toBeInTheDocument();
  expect(screen.getByRole("button",{name:/Link tracks/})).toBeDisabled();
  expect(screen.getByRole("button",{name:/Preview changes/})).toBeDisabled();
});

it("links the chosen playlist and says what happened to new tracks",async()=>{
  const rekordbox=fakeRekordbox();
  await open(rekordbox);
  const select=await screen.findByLabelText("Tracks to link");
  fireEvent.focus(select);
  await waitFor(()=>expect(rekordbox.playlists).toHaveBeenCalled());
  expect(await screen.findByRole("option",{name:"Warmup (12)"})).toBeInTheDocument();
  expect(screen.queryByRole("option",{name:/Sets/})).toBeNull();
  fireEvent.change(select,{target:{value:"p1"}});
  fireEvent.click(screen.getByRole("button",{name:/^Link tracks$/}));
  await waitFor(()=>expect(rekordbox.importTracks).toHaveBeenCalledWith(expect.any(String),{kind:"playlist",id:"p1"}));
  expect(await screen.findByText(/2 new tracks are in Incoming/)).toBeInTheDocument();
});

it("previews each field with who is newer and applies only after an explicit acknowledgement",async()=>{
  const rekordbox=fakeRekordbox();
  await open(rekordbox);
  fireEvent.click(await screen.findByRole("button",{name:/Preview changes/}));
  const tracks=within(await screen.findByRole("list",{name:"Tracks to review"}));
  expect(tracks.getByText("Write to rekordbox")).toBeInTheDocument();
  expect(tracks.getByText("rekordbox is newer")).toBeInTheDocument();
  expect(tracks.getByText("Cuerithm has no value")).toBeInTheDocument();
  expect(tracks.getByText(/Blocked: identity changed/)).toBeInTheDocument();
  const playlists=within(screen.getByRole("list",{name:"Playlists to review"}));
  expect(playlists.getByText("Sets / Friday")).toBeInTheDocument();
  expect(playlists.getByText(/Create · 12 tracks · 2 not linked to rekordbox/)).toBeInTheDocument();
  expect(playlists.getByText(/Left alone — already in rekordbox/)).toBeInTheDocument();
  expect(playlists.queryByText("Old")).toBeNull();
  expect(playlists.getByText("Smart list: Peak")).toBeInTheDocument();
  expect(playlists.getByText(/rekordbox keeps it up to date/)).toBeInTheDocument();
  expect(playlists.getByText(/written as a playlist because rekordbox cannot express a rule/)).toBeInTheDocument();
  expect(rekordbox.apply).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:/Apply 4 changes/}));
  const dialog=await screen.findByRole("alertdialog");
  const confirm=within(dialog).getByRole("button",{name:/Write 4 changes/});
  expect(confirm).toBeDisabled();
  expect(dialog).toHaveTextContent("backup");
  expect(rekordbox.apply).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("checkbox"));
  fireEvent.click(confirm);
  await waitFor(()=>expect(rekordbox.apply).toHaveBeenCalledTimes(1));
  expect(await screen.findByText(/Wrote 1 tracks to rekordbox. A verified backup was made first./)).toBeInTheDocument();
});

it("previews but offers no apply for a library whose schema is not verified for writing",async()=>{
  const rekordbox=fakeRekordbox();
  rekordbox.preflight.mockResolvedValue(preflight({write_supported:false}));
  await open(rekordbox);
  fireEvent.click(await screen.findByRole("button",{name:/Preview changes/}));
  await screen.findByRole("list",{name:"Tracks to review"});
  expect(screen.getByRole("alert",{})).toHaveTextContent("not verified for writing");
  expect(screen.queryByRole("button",{name:/Apply/})).toBeNull();
});

it("shows conflicts when rekordbox changed after reading, and that nothing was written",async()=>{
  const rekordbox=fakeRekordbox();
  rekordbox.apply.mockResolvedValue({run:null,conflicts:[{subject:"101",reason:"The track changed in rekordbox after it was last read"}]});
  await open(rekordbox);
  fireEvent.click(await screen.findByRole("button",{name:/Preview changes/}));
  fireEvent.click(await screen.findByRole("button",{name:/Apply 4 changes/}));
  const dialog=await screen.findByRole("alertdialog");
  fireEvent.click(within(dialog).getByRole("checkbox"));
  fireEvent.click(within(dialog).getByRole("button",{name:/Write 4 changes/}));
  const alert=await screen.findByText(/Nothing was written/);
  expect(alert.closest("div")).toHaveTextContent("changed in rekordbox after it was last read");
  await waitFor(()=>expect(screen.queryByRole("alertdialog")).toBeNull());
});

it("lists past syncs with the backup that was made first",async()=>{
  const rekordbox=fakeRekordbox();
  rekordbox.history.mockResolvedValue([run,{...run,id:"r0",state:"nothing_to_write",tracks:[],backup:null,journal:null,skipped:0}]);
  await open(rekordbox);
  const history=within(await screen.findByRole("region",{name:"Sync history"}));
  expect(history.getByText("Wrote 1 track")).toBeInTheDocument();
  expect(history.getByText(/Backup: \/data\/backups\/r1\/master.backup.db/)).toBeInTheDocument();
  expect(history.getByText("Nothing to write")).toBeInTheDocument();
});
