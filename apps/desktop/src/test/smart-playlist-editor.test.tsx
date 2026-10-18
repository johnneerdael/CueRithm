import {render,screen,fireEvent,waitFor} from "@testing-library/react";
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
});
