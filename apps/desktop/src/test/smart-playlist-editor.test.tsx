import {render,screen,fireEvent,waitFor,act} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import PlaylistEditor from "../views/PlaylistEditor";
import type {SmartCatalog,SmartPreview,PlaylistWorkspace} from "../playlist-types";
import {fakeBridge} from "./bridge";
const catalog:SmartCatalog={time_zone:"UTC",catalog:{fields:{title:{label:"Title",kind:"text",any_text:true,unavailable_reason:null}},tags:[],categories:{},playlists:{}},tags:[],categories:[],playlists:[],stamp:{input_revision:1,playlist_revision:1,preference_revision:1}};
const workspace:PlaylistWorkspace={tree:{revision:1,nodes:[]},statuses:[],latest_undo:null,service_error:null};
const preview:SmartPreview={id:"approved-preview",track_ids:["a"],stamp:{input_revision:1,playlist_revision:1,preference_revision:1},created:1,expires:null};
describe("smart playlist editor",()=>{
  it("previews typed rules and saves only through the pinned native preview",async()=>{
    const bridge=fakeBridge([]),saved=vi.fn(async()=>{});
    bridge.getSmartCatalog=vi.fn(async()=>catalog);bridge.previewSmartList=vi.fn(async()=>preview);
    render(<PlaylistEditor bridge={bridge} workspace={workspace} node={null} parentId={null} onSaved={saved} onClose={()=>{}}/>);
    fireEvent.change(screen.getByLabelText("Playlist type"),{target:{value:"smartlist"}});
    await screen.findByLabelText("Value for condition 1");
    fireEvent.change(screen.getByLabelText("Playlist name"),{target:{value:"Warm House"}});
    fireEvent.change(screen.getByLabelText("Value for condition 1"),{target:{value:"House"}});
    expect(screen.getByRole("button",{name:"Create"})).toBeDisabled();
    fireEvent.click(screen.getByRole("button",{name:"Preview matches"}));
    await screen.findByText("1 matching track");
    fireEvent.click(screen.getByRole("button",{name:"Create"}));
    await waitFor(()=>expect(bridge.saveSmartPlaylist).toHaveBeenCalledWith("approved-preview",null,"Warm House",null,1));
    expect(bridge.editPlaylist).not.toHaveBeenCalled();
    expect(saved).toHaveBeenCalled();
  });
  it("invalidates a preview on editing and ignores late preview results",async()=>{
    const bridge=fakeBridge([]);bridge.getSmartCatalog=vi.fn(async()=>catalog);
    let finish!:(p:SmartPreview)=>void;bridge.previewSmartList=vi.fn(async()=>new Promise<SmartPreview>(resolve=>{finish=resolve;}));
    render(<PlaylistEditor bridge={bridge} workspace={workspace} node={null} parentId={null} onSaved={async()=>{}} onClose={()=>{}}/>);
    fireEvent.change(screen.getByLabelText("Playlist type"),{target:{value:"smartlist"}});
    fireEvent.change(await screen.findByLabelText("Value for condition 1"),{target:{value:"House"}});
    fireEvent.change(screen.getByLabelText("Playlist name"),{target:{value:"House"}});
    fireEvent.click(screen.getByRole("button",{name:"Preview matches"}));
    await waitFor(()=>expect(finish).toBeDefined());
    fireEvent.change(screen.getByLabelText("Value for condition 1"),{target:{value:"Techno"}});
    finish(preview);
    await waitFor(()=>expect(screen.queryByText("1 matching track")).not.toBeInTheDocument());
    expect(screen.getByRole("button",{name:"Create"})).toBeDisabled();
  });
  it("lets the user retry a catalog failure without an automatic request loop",async()=>{
    const bridge=fakeBridge([]);bridge.getSmartCatalog=vi.fn().mockRejectedValueOnce(Error("Drive offline")).mockResolvedValue(catalog);
    render(<PlaylistEditor bridge={bridge} workspace={workspace} node={null} parentId={null} onSaved={async()=>{}} onClose={()=>{}}/>);
    fireEvent.change(screen.getByLabelText("Playlist type"),{target:{value:"smartlist"}});
    const retry=await screen.findByRole("button",{name:"Reload rule fields"});
    expect(bridge.getSmartCatalog).toHaveBeenCalledTimes(1);
    fireEvent.click(retry);await screen.findByLabelText("Value for condition 1");
    expect(bridge.getSmartCatalog).toHaveBeenCalledTimes(2);
  });
  const dateCatalog:SmartCatalog={...catalog,catalog:{...catalog.catalog,fields:{dateAdded:{label:"Date added",kind:"date",any_text:false,unavailable_reason:null}}}};
  async function dateEditor(operator:"DateEquals"|"DateBetween",bridge= fakeBridge([])){
    bridge.getSmartCatalog=vi.fn(async()=>dateCatalog);bridge.previewSmartList=vi.fn(async()=>preview);
    render(<PlaylistEditor bridge={bridge} workspace={workspace} node={null} parentId={null} onSaved={async()=>{}} onClose={()=>{}}/>);
    fireEvent.change(screen.getByLabelText("Playlist type"),{target:{value:"smartlist"}});
    await screen.findByLabelText("Date for condition 1");
    if(operator!=="DateEquals")fireEvent.change(screen.getByLabelText("Operator for condition 1"),{target:{value:operator}});
    fireEvent.change(screen.getByLabelText("Playlist name"),{target:{value:"Date list"}});
    fireEvent.click(screen.getByRole("button",{name:"Preview matches"}));
    await screen.findByText("1 matching track");
    return bridge;
  }
  it.each(["Date for condition 1","Start date for condition 1","End date for condition 1"])("blocks preview and saving when %s is cleared",async label=>{
    const bridge=await dateEditor(label.startsWith("Date ")?"DateEquals":"DateBetween");
    fireEvent.change(screen.getByLabelText(label),{target:{value:""}});
    expect(screen.getByRole("button",{name:"Create"})).toBeDisabled();
    expect(screen.getByRole("button",{name:"Preview matches"})).toBeDisabled();
    fireEvent.click(screen.getByRole("button",{name:"Preview matches"}));
    expect(bridge.previewSmartList).toHaveBeenCalledTimes(1);
    expect(bridge.saveSmartPlaylist).not.toHaveBeenCalled();
  });
  it.each(["Date for condition 1","Start date for condition 1","End date for condition 1"])("blocks a rejected %s until a valid replacement resolves",async label=>{
    const bridge=fakeBridge([]);bridge.resolvePlaylistDate=vi.fn().mockRejectedValueOnce(Error("Date unavailable")).mockResolvedValueOnce(Date.UTC(2026,9,17));
    await dateEditor(label.startsWith("Date ")?"DateEquals":"DateBetween",bridge);
    fireEvent.change(screen.getByLabelText(label),{target:{value:"2026-10-16"}});
    await screen.findByText("Date unavailable");
    expect(screen.getByRole("button",{name:"Preview matches"})).toBeDisabled();
    expect(screen.getByRole("button",{name:"Create"})).toBeDisabled();
    fireEvent.change(screen.getByLabelText(label),{target:{value:"2026-10-17"}});
    await waitFor(()=>expect(screen.getByRole("button",{name:"Preview matches"})).toBeEnabled());
    expect(screen.getByRole("button",{name:"Create"})).toBeDisabled();
    fireEvent.click(screen.getByRole("button",{name:"Preview matches"}));
    await screen.findByText("1 matching track");
    const rule=vi.mocked(bridge.previewSmartList).mock.calls.at(-1)![0].rules[0];
    expect(rule.value).toEqual(label.startsWith("Date ")?{kind:"date",value:Date.UTC(2026,9,17)}:{kind:"date_range",value:expect.objectContaining({[label.startsWith("Start")?"start":"end"]:Date.UTC(2026,9,17)})});
  });
  it("ignores a late date resolution after clearing the input",async()=>{
    const bridge=fakeBridge([]);let finish!:(value:number)=>void;
    bridge.resolvePlaylistDate=vi.fn(async()=>new Promise<number>(resolve=>{finish=resolve;}));
    await dateEditor("DateEquals",bridge);
    fireEvent.change(screen.getByLabelText("Date for condition 1"),{target:{value:"2026-10-16"}});
    fireEvent.change(screen.getByLabelText("Date for condition 1"),{target:{value:""}});
    await act(async()=>finish(Date.UTC(2026,9,16)));
    expect(screen.getByLabelText("Date for condition 1")).toHaveValue("");
    expect(screen.getByRole("button",{name:"Preview matches"})).toBeDisabled();
  });
  it("releases removed date requests and ignores their late completion after an operator change",async()=>{
    const bridge=fakeBridge([]);let finish!:(value:number)=>void;
    bridge.resolvePlaylistDate=vi.fn(async()=>new Promise<number>(resolve=>{finish=resolve;}));
    await dateEditor("DateEquals",bridge);
    fireEvent.change(screen.getByLabelText("Date for condition 1"),{target:{value:"2026-10-16"}});
    fireEvent.change(screen.getByLabelText("Operator for condition 1"),{target:{value:"DateBefore"}});
    expect(screen.getByRole("button",{name:"Preview matches"})).toBeEnabled();
    await act(async()=>finish(Date.UTC(2026,9,16)));
    fireEvent.click(screen.getByRole("button",{name:"Preview matches"}));
    await screen.findByText("1 matching track");
    expect(vi.mocked(bridge.previewSmartList).mock.calls.at(-1)![0].rules[0].value).not.toEqual({kind:"date",value:Date.UTC(2026,9,16)});
  });

  it.each(["success","failure"])("keeps the latest date usable when a superseded request ends in %s",async outcome=>{
    const bridge=fakeBridge([]),requests=new Map<string,{resolve:(v:number)=>void;reject:(e:Error)=>void}>();
    bridge.resolvePlaylistDate=vi.fn(async day=>new Promise<number>((resolve,reject)=>{requests.set(day,{resolve,reject});}));
    await dateEditor("DateEquals",bridge);
    fireEvent.change(screen.getByLabelText("Date for condition 1"),{target:{value:"2026-10-16"}});
    fireEvent.change(screen.getByLabelText("Date for condition 1"),{target:{value:"2026-10-17"}});
    await act(async()=>requests.get("2026-10-17")!.resolve(Date.UTC(2026,9,17)));
    expect(screen.getByRole("button",{name:"Preview matches"})).toBeEnabled();
    fireEvent.click(screen.getByRole("button",{name:"Preview matches"}));
    await screen.findByText("1 matching track");
    await act(async()=>{const old=requests.get("2026-10-16")!;if(outcome==="success")old.resolve(Date.UTC(2026,9,16));else old.reject(Error("Stale error"));});
    expect(screen.getByLabelText("Date for condition 1")).toHaveValue("2026-10-17");
    expect(screen.queryByText("Stale error")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Create"})).toBeEnabled();
  });
  it("keeps a cleared range endpoint invalid when the other endpoint resolves",async()=>{
    const bridge=fakeBridge([]);let finish!:(value:number)=>void;
    bridge.resolvePlaylistDate=vi.fn(async()=>new Promise<number>(resolve=>{finish=resolve;}));
    await dateEditor("DateBetween",bridge);
    fireEvent.change(screen.getByLabelText("Start date for condition 1"),{target:{value:""}});
    fireEvent.change(screen.getByLabelText("End date for condition 1"),{target:{value:"2026-10-17"}});
    await act(async()=>finish(Date.UTC(2026,9,17)));
    expect(screen.getByLabelText("Start date for condition 1")).toHaveValue("");
    expect(screen.getByRole("button",{name:"Preview matches"})).toBeDisabled();
  });

});
