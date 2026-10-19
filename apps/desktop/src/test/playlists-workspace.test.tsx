import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import type {TrackQuery,TrackPage} from "../types";
import type {PlaylistWorkspace} from "../playlist-types";

function playlists():PlaylistWorkspace{return {tree:{revision:1,nodes:[
  {id:"house",parent_id:null,kind:"playlist",name:"House",position:0,favorite_position:0,definition:null,deleted:false,deleted_by:null},
  {id:"techno",parent_id:null,kind:"playlist",name:"Techno",position:1,favorite_position:null,definition:null,deleted:false,deleted_by:null},
]},statuses:[],latest_undo:null,service_error:null};}

describe("playlist workspace",()=>{
  it("scopes the table while preserving the loaded player and clears old track selection",async()=>{
    const bridge=Object.assign(fakeBridge([track("a"),track("b")]),{getPlaylists:vi.fn(async()=>playlists())});
    bridge.listTracks=vi.fn(async query=>{const q=query as TrackQuery&{playlist_id?:string|null};const rows=q.playlist_id==="house"?[track("a")]:bridge.rows;return {rows,total:rows.length,revision:1,next_cursor:null};});
    render(<App bridge={bridge}/>);
    await screen.findByText("Track b");
    fireEvent.doubleClick(screen.getByText("Track a"));
    await waitFor(()=>expect(bridge.playerLoad).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByText("Track b"));
    await screen.findByText("1 selected");
    fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));
    await screen.findByRole("heading",{name:"House 1"});
    await waitFor(()=>expect(screen.queryByText("Track b")).not.toBeInTheDocument());
    expect(screen.queryByText("1 selected")).not.toBeInTheDocument();
    expect(bridge.listTracks).toHaveBeenLastCalledWith(expect.objectContaining({playlist_id:"house",sort:"playlist_order"}));
    expect(bridge.playerLoad).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button",{name:"Pause"})).toBeEnabled();
  });
  it("does not let late rows or select-all results cross a playlist boundary",async()=>{
    const bridge=Object.assign(fakeBridge([]),{getPlaylists:vi.fn(async()=>playlists())});
    let finishHouse!:(value:Awaited<ReturnType<typeof bridge.listTracks>>)=>void;
    bridge.listTracks=vi.fn(async (q:TrackQuery)=>{if(q.playlist_id==="house")return new Promise<TrackPage>(resolve=>{finishHouse=resolve;});const rows=q.playlist_id==="techno"?[track("b")]:[];return {rows,total:rows.length,revision:1,next_cursor:null};});
    let finishSelection!:(ids:string[])=>void;
    bridge.selectTrackIds=vi.fn(async()=>new Promise<string[]>(resolve=>{finishSelection=resolve;}));
    render(<App bridge={bridge}/>);
    fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));
    await waitFor(()=>expect(finishHouse).toBeDefined());
    fireEvent.keyDown(window,{key:"a",metaKey:true});
    await waitFor(()=>expect(finishSelection).toBeDefined());
    fireEvent.click(screen.getByRole("button",{name:"Playlist Techno"}));
    await screen.findByText("Track b");
    finishHouse({rows:[track("a")],total:1,revision:1,next_cursor:null});finishSelection(["a"]);
    await waitFor(()=>expect(screen.queryByText("Track a")).not.toBeInTheDocument());
    expect(screen.queryByText("1 selected")).not.toBeInTheDocument();
  });
  it("removes old rows immediately while the next playlist is loading",async()=>{
    const bridge=fakeBridge([track("a")]);bridge.getPlaylists=vi.fn(async()=>playlists());
    let finish!:(page:TrackPage)=>void;
    bridge.listTracks=vi.fn(async q=>q.playlist_id==="techno"?new Promise<TrackPage>(resolve=>{finish=resolve;}):{rows:[track("a")],total:1,revision:1,next_cursor:null});
    render(<App bridge={bridge}/>);await screen.findByText("Track a");
    fireEvent.click(await screen.findByRole("button",{name:"Playlist Techno"}));
    expect(screen.queryByText("Track a")).not.toBeInTheDocument();
    finish({rows:[track("b")],total:1,revision:1,next_cursor:null});await screen.findByText("Track b");
  });
  it("creates a folder through the native edit API and opens its scope",async()=>{
    const bridge=fakeBridge([]);let workspace=playlists();workspace.tree.nodes=[];
    bridge.getPlaylists=vi.fn(async()=>workspace);
    bridge.editPlaylist=vi.fn(async edit=>{
      if(edit.type!=="create")throw Error("Unexpected edit");
      workspace={...workspace,tree:{revision:2,nodes:[{id:"new",parent_id:edit.parent_id,kind:edit.kind,name:edit.name,position:0,favorite_position:null,definition:edit.definition,deleted:false,deleted_by:null}]}};
      return {edit_id:"create",revision:2,created_ids:["new"],affected_ids:["new"]};
    });
    render(<App bridge={bridge}/>);
    const create=await screen.findByRole("button",{name:"Create playlist"});await waitFor(()=>expect(create).toBeEnabled());fireEvent.click(create);
    const dialog=await screen.findByRole("dialog",{name:"Create playlist"});
    fireEvent.change(screen.getByLabelText("Playlist type"),{target:{value:"folder"}});
    fireEvent.change(screen.getByLabelText("Playlist name"),{target:{value:"New set"}});
    fireEvent.click(screen.getByRole("button",{name:"Create"}));
    await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenCalledWith(expect.objectContaining({type:"create",kind:"folder",name:"New set"}),1));
    await waitFor(()=>expect(dialog).not.toBeInTheDocument());
    await screen.findByRole("button",{name:"Folder New set"});
  });
});
