import {render,screen,fireEvent,waitFor,act} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import PublicationEditor from "../components/PublicationEditor";
import type {ManualPublicationApi,ManualPublicationSnapshot,TemplatePublicationRequest} from "../manual-publication-types";
const snapshot=():ManualPublicationSnapshot=>({guard:{track_id:"a",source_sha256:"source",entity_epoch:5,cue_revision_id:"cue-1"},library_revision:8,duration_seconds:120,publication:{grid:{bpm:120,startTime:0},cues:[{name:"Start",startTime:0,type:"1",position:0,color:"yellow"}],implementation_sha256:"measured",inputs_sha256:"original"}});
function api():ManualPublicationApi {
 const initial=snapshot();
 return {snapshot:vi.fn(async()=>initial),templates:vi.fn(async()=>[]),history:vi.fn(async()=>[]),preview:vi.fn(async request=>({id:"preview",library_revision:request.expected_revision,changes:[{track_id:"a",before:initial.publication,after:{...initial.publication,grid:request.grid,cues:request.cues},preserved_manual:false}]})),apply:vi.fn(async()=>({id:"edit",track_ids:["a"],library_revision:9})),undo:vi.fn(async()=>({id:"edit",track_ids:["a"],library_revision:10})),saveTemplate:vi.fn(),deleteTemplate:vi.fn(),previewTemplate:vi.fn()};
}
it("previews corrected grid and loops before explicit save and keeps raw playback untouched",async()=>{
 const bridge=api();const changed=vi.fn();render(<PublicationEditor trackId="a" api={bridge} position={8} onSeek={vi.fn()} onPublicationChanged={changed}/>);
 await screen.findByLabelText("Grid BPM");fireEvent.click(screen.getByRole("button",{name:"Half BPM"}));expect(screen.getByLabelText("Grid BPM")).toHaveValue(60);
 fireEvent.click(screen.getByRole("button",{name:"Add saved loop at playhead"}));expect(screen.getByLabelText("Loop end 2")).toHaveValue(24);
 fireEvent.click(screen.getByRole("button",{name:"Preview corrections"}));await screen.findByRole("button",{name:"Save preview"});expect(bridge.apply).not.toHaveBeenCalled();
 expect(bridge.preview).toHaveBeenCalledWith(expect.objectContaining({grid:{bpm:60,startTime:0},guard:snapshot().guard,cues:expect.arrayContaining([expect.objectContaining({startTime:8,endTime:24,type:"5"})])}));
 fireEvent.click(screen.getByRole("button",{name:"Save preview"}));await waitFor(()=>expect(changed).toHaveBeenCalledWith(expect.objectContaining({id:"edit"})));expect(bridge.apply).toHaveBeenCalledWith("preview",8);
});
it("retains edits after conflict, invalidates preview after editing and explains saved loop playback",async()=>{
 const bridge=api();bridge.apply=vi.fn(async()=>{throw Error("Track changed; refresh the correction preview");});render(<PublicationEditor trackId="a" api={bridge} position={2} onSeek={vi.fn()}/>);
 await screen.findByLabelText("Cue name 1");fireEvent.change(screen.getByLabelText("Cue name 1"),{target:{value:"My intro"}});fireEvent.click(screen.getByRole("button",{name:"Preview corrections"}));await screen.findByRole("button",{name:"Save preview"});fireEvent.click(screen.getByRole("button",{name:"Save preview"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Track changed");expect(screen.getByLabelText("Cue name 1")).toHaveValue("My intro");
 fireEvent.change(screen.getByLabelText("Cue name 1"),{target:{value:"My new intro"}});expect(screen.queryByRole("button",{name:"Save preview"})).not.toBeInTheDocument();expect(screen.getByText(/Local preview seeks to loop starts/)).toBeInTheDocument();
});
it("creates named templates with overwrite off and previews selected tracks with manual preservation",async()=>{
 const bridge=api();bridge.saveTemplate=vi.fn(async(_id,name,template)=>({id:"t",name,template}));bridge.previewTemplate=vi.fn(async (request:TemplatePublicationRequest)=>({id:"batch",library_revision:request.expected_revision,changes:[{track_id:"a",before:snapshot().publication,after:snapshot().publication,preserved_manual:true}]}));render(<PublicationEditor trackId="a" api={bridge} position={0} onSeek={vi.fn()}/>);
 await screen.findByLabelText("Grid BPM");fireEvent.click(screen.getByRole("button",{name:"New cue template"}));fireEvent.change(screen.getByLabelText("Template name"),{target:{value:"Home practice"}});expect(screen.getByLabelText("Replace existing cue slots")).not.toBeChecked();fireEvent.click(screen.getByRole("button",{name:"Save template"}));
 await waitFor(()=>expect(bridge.saveTemplate).toHaveBeenCalledWith(null,"Home practice",expect.objectContaining({overwrite:false}),8));
});

it("previews an explicit selected batch and keeps preservation on until changed",async()=>{
 const bridge=api();bridge.snapshot=vi.fn(async id=>({...snapshot(),guard:{...snapshot().guard,track_id:id}}));
 const template={overwrite:false,startCueBehavior:"atFirstBeat" as const,breakdownMinBeats:64,keepCuePositions:false,enableCustomAnchors:false,dropAtStart:"never",customAnchors:{},rows:[]};
 bridge.templates=vi.fn(async()=>[{id:"practice",name:"Practice",template}]);bridge.previewTemplate=vi.fn(async (request:TemplatePublicationRequest)=>({id:"batch",library_revision:request.expected_revision,changes:request.guards.map(guard=>({track_id:guard.track_id,before:snapshot().publication,after:snapshot().publication,preserved_manual:true}))}));
 render(<PublicationEditor trackId="a" api={bridge} position={0} onSeek={vi.fn()} selectedTrackIds={["a","b"]}/>);
 await screen.findByLabelText("Saved cue template");expect(screen.getByLabelText("Preserve manual corrections")).toBeChecked();
 fireEvent.click(screen.getByLabelText("Apply to 2 selected tracks"));fireEvent.click(screen.getByRole("button",{name:"Preview template"}));
 await screen.findByRole("region",{name:"Correction preview"});expect(bridge.previewTemplate).toHaveBeenCalledWith({template_id:"practice",guards:[expect.objectContaining({track_id:"a"}),expect.objectContaining({track_id:"b"})],expected_revision:8,preserve_manual:true});expect(bridge.apply).not.toHaveBeenCalled();
});

it("ignores a late snapshot when selection moves A to B to A",async()=>{
 const bridge=api();let resolveOld!:(value:ManualPublicationSnapshot)=>void;
 const old=new Promise<ManualPublicationSnapshot>(resolve=>{resolveOld=resolve;});let calls=0;
 bridge.snapshot=vi.fn(async id=>{calls++;if(calls===1)return old;const value=snapshot();value.guard.track_id=id;value.publication.cues[0].name=calls===2?"Track B":"Current A";return value;});
 const props={api:bridge,position:0,onSeek:vi.fn()};const {rerender}=render(<PublicationEditor trackId="a" {...props}/>);
 rerender(<PublicationEditor trackId="b" {...props}/>);await screen.findByDisplayValue("Track B");rerender(<PublicationEditor trackId="a" {...props}/>);await screen.findByDisplayValue("Current A");
 const stale=snapshot();stale.publication.cues[0].name="Old A";await act(async()=>{resolveOld(stale);});await waitFor(()=>expect(screen.getByLabelText("Cue name 1")).toHaveValue("Current A"));
});
