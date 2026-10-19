import {useState} from "react";
import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import SmartRuleEditor from "../components/SmartRuleEditor";
import type {SmartCatalog,SmartDefinition} from "../playlist-types";
import {fakeBridge} from "./bridge";
const descriptor=(kind:string,label:string,unavailable_reason:string|null=null)=>({kind,label,unavailable_reason,any_text:kind==="text"});
const catalog:SmartCatalog={time_zone:"America/Los_Angeles",catalog:{fields:{title:descriptor("text","Title"),bpm:descriptor("number","BPM"),dateAdded:descriptor("date","Date added"),tags:descriptor("tags","Tags"),categories:descriptor("categories","Tag categories"),playlist:descriptor("playlist","Playlist"),cloudUpload:descriptor("boolean","Cloud upload","No cloud service")},tags:["dark"],categories:{mood:["dark"]},playlists:{}},tags:[{id:"dark",category_id:"mood",name:"Dark"}],categories:[{id:"mood",name:"Mood"}],playlists:[],stamp:{input_revision:1,playlist_revision:1,preference_revision:5}};
function Harness({bridge,initial,onChange}:{bridge:ReturnType<typeof fakeBridge>;initial:SmartDefinition;onChange:(d:SmartDefinition)=>void}){
  const [value,setValue]=useState(initial);
  return <SmartRuleEditor bridge={bridge} catalog={catalog} value={value} onChange={next=>{setValue(next);onChange(next);}} onPending={()=>{}} onError={()=>{}}/>;
}
describe("typed smart rules",()=>{
  it("edits Any/All, numeric ranges and source OR flags without a JSON editor",async()=>{
    const changed=vi.fn();render(<Harness bridge={fakeBridge([])} initial={{match_all:true,rules:[{id:"r",field:"title",operator:"StringContains",value:{kind:"text",value:""},or:false}]}} onChange={changed}/>);
    fireEvent.change(screen.getByLabelText("Match conditions"),{target:{value:"any"}});
    fireEvent.change(screen.getByLabelText("Field for condition 1"),{target:{value:"bpm"}});
    fireEvent.change(screen.getByLabelText("Operator for condition 1"),{target:{value:"NumberBetween"}});
    fireEvent.change(screen.getByLabelText("Minimum for condition 1"),{target:{value:"120"}});
    fireEvent.change(screen.getByLabelText("Maximum for condition 1"),{target:{value:"130"}});
    fireEvent.click(screen.getByLabelText("OR group for condition 1"));
    expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({match_all:false,rules:[expect.objectContaining({field:"bpm",operator:"NumberBetween",value:{kind:"number_range",value:{start:120,end:130}},or:true})]}));
    expect(screen.queryByRole("textbox",{name:/JSON/})).not.toBeInTheDocument();
    expect(screen.getByRole("option",{name:/Cloud upload/})).toBeDisabled();
  });
  it("selects real tags and resolves calendar days through the native library time zone",async()=>{
    const bridge=fakeBridge([]),changed=vi.fn();bridge.resolvePlaylistDate=vi.fn(async()=>1792220400000);
    render(<Harness bridge={bridge} initial={{match_all:true,rules:[{id:"r",field:"tags",operator:"TagsAtleastOne",value:{kind:"ids",value:[]},or:false}]}} onChange={changed}/>);
    fireEvent.click(screen.getByLabelText("Dark"));
    expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({rules:[expect.objectContaining({value:{kind:"ids",value:["dark"]}})]}));
    fireEvent.change(screen.getByLabelText("Field for condition 1"),{target:{value:"dateAdded"}});
    fireEvent.change(screen.getByLabelText("Date for condition 1"),{target:{value:"2026-10-17"}});
    await waitFor(()=>expect(bridge.resolvePlaylistDate).toHaveBeenCalledWith("2026-10-17",5));
    await waitFor(()=>expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({rules:[expect.objectContaining({value:{kind:"date",value:1792220400000}})]})));
  });
  it("merges out-of-order start/end date resolutions without losing either edit",async()=>{
    const bridge=fakeBridge([]),changed=vi.fn(),pending=new Map<string,(v:number)=>void>();
    bridge.resolvePlaylistDate=vi.fn(async day=>new Promise<number>(resolve=>{pending.set(day,resolve);}));
    render(<Harness bridge={bridge} initial={{match_all:true,rules:[{id:"r",field:"dateAdded",operator:"DateBetween",value:{kind:"date_range",value:{start:Date.UTC(2026,0,1),end:Date.UTC(2026,1,1)}},or:false}]}} onChange={changed}/>);
    fireEvent.change(screen.getByLabelText("Start date for condition 1"),{target:{value:"2026-10-17"}});
    fireEvent.change(screen.getByLabelText("End date for condition 1"),{target:{value:"2026-10-19"}});
    pending.get("2026-10-19")!(1792393200000);
    await waitFor(()=>expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({rules:[expect.objectContaining({value:{kind:"date_range",value:expect.objectContaining({end:1792393200000})}})]})));
    pending.get("2026-10-17")!(1792220400000);
    await waitFor(()=>expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({rules:[expect.objectContaining({value:{kind:"date_range",value:{start:1792220400000,end:1792393200000}}})]})));
  });
});
