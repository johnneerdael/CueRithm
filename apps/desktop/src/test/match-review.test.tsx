import {act,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import MatchReview from "../components/MatchReview";
import {fakeBridge,track} from "./bridge";
import type {MatchReviewState,MatchReviewPreview,MatchReviewRequest} from "../match-review-types";
import {nativeValues} from "./editor-fixtures";
import {inspectorOperation} from "../inspector-operations";
function fixture(){
 const bridge=fakeBridge([track("a")]);let count=0;
 const review:MatchReviewState={track_id:"a",track_state:"incoming",job_id:"same-job",attempt:2,generation:"generation",source_sha256:"source",settings_revision:"settings",status:"ambiguous",source:{title:"Borrowed Light",artists:["N. Vale"],album:null,version:"Club Mix",duration_seconds:402,has_artwork:false},candidates:[{id:"club",provider:"Beatport",track_id:"11",release_id:"22",title:"Borrowed Light",artists:["Nora Vale"],album:"Quiet Forms",version:"Club Mix",isrc:"TEST-ISRC",duration_seconds:402,similarity:.94,identity_conflicts:[],source_url:"https://www.beatport.com/track/example/11",artwork_url:"https://images.beatport.com/example.jpg",selectable:true,reason:null},{id:"dub",provider:"Beatport",track_id:"12",release_id:"23",title:"Borrowed Light",artists:["Nora Vale"],album:"Quiet Forms",version:"Dub Mix",isrc:"OTHER-ISRC",duration_seconds:446,similarity:.96,identity_conflicts:["Version differs: Club Mix / Dub Mix"],source_url:"javascript:alert(1)",artwork_url:null,selectable:true,reason:null}],truncated:false,can_requery:true,can_keep_existing_once:true,providers:[],warnings:[],reason:"Confirm the recording and edition."};
 bridge.trackMatchReview=vi.fn(async()=>review);
 bridge.previewMatchReview=vi.fn(async(_operationId:string,request:MatchReviewRequest)=>{const candidate=review.candidates.find(c=>c.id===request.candidate_id)??null;const action=request.fields.find(field=>field.field==="Title")?.action??"keep";const preview:MatchReviewPreview={id:"preview-"+(++count),revision:"revision-"+count,track_id:"a",job_id:"same-job",attempt:2,generation:request.generation,action:request.action,candidate,requires_id3_upgrade:false,can_apply:!candidate?.identity_conflicts.length||request.acknowledge_conflicts,blockers:candidate?.identity_conflicts.length&&!request.acknowledge_conflicts?["Acknowledge the edition conflict"]:[],fields:candidate?[{field:"Title",label:"Title",before:["Borrowed Light"],proposed:["Borrowed Light ("+candidate.version+")"],after:action==="keep"?["Borrowed Light"]:["Borrowed Light ("+candidate.version+")"],action,allowed_actions:["keep","replace"],reason:null}]:[],artwork:{existing:false,candidate_url:candidate?.artwork_url??null,action:"keep",reason:null}};return preview;});
 bridge.applyMatchReview=vi.fn(async()=>({state:"queued",job_id:"same-job",metadata_batch:null}));
 bridge.matchReviewArtwork=vi.fn(async()=>({id:"safe-asset",mime:"image/png",sha256:"image",bytes:8}));bridge.metadataArtworkAsset=vi.fn(async()=>"data:image/png;base64,iVBORw0KGgo=");
 return {bridge,review};
}
describe("match review evidence and decisions",()=>{
 it("opens a source through native provider validation without choosing a candidate or propagating to automatic links",async()=>{
  const {bridge,review}=fixture();review.candidates[0].provider="beatport";const automatic=vi.fn();window.addEventListener("click",automatic);
  try{
   render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);const source=await screen.findByRole("link",{name:"Source page"});expect(fireEvent.click(source)).toBe(false);
   await waitFor(()=>expect(bridge.openCatalogSource).toHaveBeenCalledExactlyOnceWith("beatport","https://www.beatport.com/track/example/11"));expect(automatic).not.toHaveBeenCalled();expect(screen.getByRole("radio",{name:"beatport · Borrowed Light · Club Mix"})).not.toBeChecked();expect(bridge.previewMatchReview).not.toHaveBeenCalled();expect(bridge.playerLoad).not.toHaveBeenCalled();
  }finally{window.removeEventListener("click",automatic);}
 });
 it("keeps scoped MusicBrainz edition dates and credits visible as read-only evidence",async()=>{
  const {bridge,review}=fixture();review.candidates.splice(1);const chosen=review.candidates[0];chosen.provider="musicbrainz";
  Object.assign(chosen,{facts:{kind:"musicbrainz",data:{schema_version:1,recording_id:"11111111-1111-4111-8111-111111111111",release_id:"22222222-2222-4222-8222-222222222222",release_group_id:null,release_track_id:"33333333-3333-4333-8333-333333333333",scope:"release_track",recording_title:"Borrowed Light",track_title:"Borrowed Light",recording_disambiguation:null,recording_credit:[{artist_id:"44444444-4444-4444-8444-444444444444",name:"Nora Vale",credited_name:"N. Vale",join_phrase:" feat. "},{artist_id:"55555555-5555-4555-8555-555555555555",name:"Miro",credited_name:"Miro",join_phrase:""}],track_credit:[],release_credit:[],isrcs:[],recording_length_ms:null,track_length_ms:null,selected_release_date:{year:2024,month:6,day:null},recording_first_release_date:{year:2020,month:null,day:null},release_group_first_release_date:null,release_events:[],position:{medium_position:1,track_position:1,printed_number:"A1",medium_track_count:2,medium_count:1},release:{title:"Quiet Forms",status:"Official",country:"BE",barcode:null,media_format:"Vinyl",labels:[]},cover:null,source_policy:{adapter_revision:"fixture",policy_revision:"fixture",data_class:"core"}}}});
  render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);fireEvent.click(await screen.findByRole("radio",{name:"musicbrainz · Borrowed Light · Club Mix"}));
  const evidence=await screen.findByRole("region",{name:"Recording and release evidence"});expect(evidence).toHaveTextContent("2024-06");expect(evidence).toHaveTextContent("N. Vale feat. Miro");expect(evidence).toHaveTextContent("A1");expect(evidence).toHaveTextContent("read-only");expect(screen.queryByRole("combobox",{name:"MusicBrainz recording ID match action"})).not.toBeInTheDocument();expect(bridge.playerLoad).not.toHaveBeenCalled();
 });
 it("refreshes unavailable source access without erasing choices or re-expanding a frozen preview",async()=>{
  const {bridge,review}=fixture();let access:()=>void=()=>{};
  bridge.onCatalogAccessChange=vi.fn(async listener=>{access=listener;return()=>{};});
  render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("radio",{name:"Beatport · Borrowed Light · Club Mix"}));
  fireEvent.change(await screen.findByRole("combobox",{name:"Title match action"}),{target:{value:"replace"}});
  await waitFor(()=>expect(screen.getByRole("button",{name:"Use selected match"})).toBeEnabled());
  const previews=vi.mocked(bridge.previewMatchReview).mock.calls.length;
  bridge.trackMatchReview=vi.fn(async():Promise<MatchReviewState>=>({...review,can_requery:false,providers:[{id:"beatport",available:false,requires_solver:true,reason:"A compatible solver is required"}],warnings:[{provider:"beatport",kind:"unavailable",message:"Solver could not retrieve the catalog page"}]}));
  act(()=>access());await screen.findByText("Solver could not retrieve the catalog page");
  expect(screen.getByRole("radio",{name:"Beatport · Borrowed Light · Club Mix"})).toBeChecked();
  expect(screen.getByRole("combobox",{name:"Title match action"})).toHaveValue("replace");
  expect(screen.getByRole("button",{name:"Requery"})).toBeDisabled();
  expect(screen.getByRole("button",{name:"Keep existing once"})).toBeEnabled();
  expect(screen.getByRole("button",{name:"Use selected match"})).toBeEnabled();
  expect(bridge.previewMatchReview).toHaveBeenCalledTimes(previews);
 });
 it("cancels a queued decision before it acquires the native operation lease",async()=>{
  const {bridge}=fixture();let release!:()=>void;
  const blocking=inspectorOperation(bridge,()=>new Promise<void>(resolve=>{release=resolve;}));
  render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Keep existing once"}));
  fireEvent.click(screen.getByRole("button",{name:"Cancel operation"}));
  await act(async()=>{release();await blocking;});
  await screen.findByText("Operation cancelled before it started.");
  expect(bridge.previewMatchReview).not.toHaveBeenCalled();expect(bridge.applyMatchReview).not.toHaveBeenCalled();
 });
 it("requires explicit ID3v2.4 conversion consent in the reviewed native request",async()=>{
  const {bridge}=fixture(),original=bridge.previewMatchReview;
  bridge.previewMatchReview=vi.fn(async(...args:Parameters<typeof original>)=>{const value=await original(...args);return {...value,requires_id3_upgrade:true,can_apply:args[1].allow_id3_upgrade,blockers:args[1].allow_id3_upgrade?[]:["Ordered contributors require ID3v2.4"]};});
  render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("radio",{name:"Beatport · Borrowed Light · Club Mix"}));
  const permission=await screen.findByRole("checkbox",{name:"Allow conversion to ID3v2.4"});expect(permission).not.toBeChecked();
  expect(screen.getByRole("button",{name:"Use selected match"})).toBeDisabled();fireEvent.click(permission);
  await waitFor(()=>expect(bridge.previewMatchReview).toHaveBeenLastCalledWith(expect.any(String),expect.objectContaining({allow_id3_upgrade:true})));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Use selected match"})).toBeEnabled());
  fireEvent.click(screen.getByRole("radio",{name:"Beatport · Borrowed Light · Dub Mix"}));
  await waitFor(()=>expect(bridge.previewMatchReview).toHaveBeenLastCalledWith(expect.any(String),expect.objectContaining({candidate_id:"dub",allow_id3_upgrade:false})));
 });
 it("keeps fields by default and applies explicit choices through a frozen preview",async()=>{
  const {bridge}=fixture(),changed=vi.fn();render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={changed}/>);
  fireEvent.click(await screen.findByRole("radio",{name:"Beatport · Borrowed Light · Club Mix"}));
  const field=await screen.findByRole("combobox",{name:"Title match action"});expect(field).toHaveValue("keep");
  fireEvent.change(field,{target:{value:"replace"}});
  await waitFor(()=>expect(bridge.previewMatchReview).toHaveBeenLastCalledWith(expect.any(String),expect.objectContaining({candidate_id:"club",fields:[{field:"Title",action:"replace"}]})));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Use selected match"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Use selected match"}));
  await screen.findByText("Preparation was queued");expect(bridge.applyMatchReview).toHaveBeenCalledWith(expect.any(String),"preview-2","revision-2");expect(bridge.playerLoad).not.toHaveBeenCalled();expect(changed).toHaveBeenCalledTimes(1);
 });
 it("waits for a cancelled preview to settle and never accepts it for another candidate",async()=>{
  const {bridge}=fixture(),original=bridge.previewMatchReview;let release!:()=>void;
  bridge.previewMatchReview=vi.fn().mockImplementationOnce(async(...args:Parameters<typeof original>)=>{const preview=await original(...args);return new Promise<MatchReviewPreview>(resolve=>{release=()=>resolve(preview);});}).mockImplementation(original);
  render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("radio",{name:"Beatport · Borrowed Light · Club Mix"}));await waitFor(()=>expect(bridge.previewMatchReview).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("radio",{name:"Beatport · Borrowed Light · Dub Mix"}));
  await waitFor(()=>expect(bridge.cancelMetadataOperation).toHaveBeenCalled());expect(bridge.previewMatchReview).toHaveBeenCalledTimes(1);
  await act(async()=>{release();});
  await screen.findByText("Acknowledge the edition conflict");expect(screen.getByRole("button",{name:"Use selected match"})).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox",{name:/I reviewed the recording/}));await waitFor(()=>expect(screen.getByRole("button",{name:"Use selected match"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Use selected match"}));await screen.findByText("Preparation was queued");expect(bridge.applyMatchReview).toHaveBeenLastCalledWith(expect.any(String),"preview-3","revision-3");
 });
 it("uses validated native artwork assets and never places provider URLs into image src",async()=>{
  const {bridge}=fixture();render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("radio",{name:"Beatport · Borrowed Light · Club Mix"}));
  fireEvent.click(await screen.findByRole("button",{name:"Preview candidate artwork"}));
  expect((await screen.findByRole("img",{name:"Candidate artwork"})).getAttribute("src")).toMatch(/^data:image\//);
  expect(bridge.matchReviewArtwork).toHaveBeenCalledWith(expect.any(String),expect.stringMatching(/^preview-/),expect.stringMatching(/^revision-/));
  fireEvent.click(screen.getByRole("radio",{name:"Beatport · Borrowed Light · Dub Mix"}));
  expect(screen.queryByRole("link",{name:"Source page"})?.getAttribute("href")).not.toMatch(/^javascript:/);
  expect(screen.queryByRole("img",{name:"Candidate artwork"})).not.toBeInTheDocument();
 });
 it("shows actual partial Ready publication outcomes instead of a queued success",async()=>{
  const {bridge,review}=fixture();review.track_state="ready";review.can_keep_existing_once=false;
  bridge.applyMatchReview=vi.fn(async()=>({state:"partial",job_id:null,metadata_batch:{id:"batch",draft_id:"draft",draft_revision:2,state:"partial",created:1,can_undo:false,can_recover:false,error:"One field could not be written",rows:[{track_id:"a",title:"Borrowed Light",before:nativeValues("Old"),after:nativeValues("Old"),outcome:"failed",fields:[{field:"title",status:"failed",reason:"Source is read only"}],error:"Source is read only"}]}}));
  render(<MatchReview bridge={bridge} trackId="a" onClose={()=>{}} onPending={()=>{}} onChanged={()=>{}}/>);
  fireEvent.click(await screen.findByRole("radio",{name:"Beatport · Borrowed Light · Club Mix"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Use selected match"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Use selected match"}));
  await screen.findByText("One field could not be written");expect(screen.getAllByText(/Source is read only/).length).toBeGreaterThan(0);expect(screen.queryByText("Preparation was queued")).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:"Keep existing once"})).not.toBeInTheDocument();
 });
});
