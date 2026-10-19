import {render,screen,fireEvent,waitFor,within} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {pointerDrag} from "./pointer-drag";
import {fakeBridge,track} from "./bridge";
import type {PlaylistWorkspace,PlaylistNode,PlaylistChange} from "../playlist-types";

const node=(id:string,name:string,extra:Partial<PlaylistNode>={}):PlaylistNode=>({id,name,kind:"playlist",parent_id:null,position:0,favorite_position:null,definition:null,deleted:false,deleted_by:null,...extra});
function setup(nodes:PlaylistNode[],undo:string|null=null){
  let workspace:PlaylistWorkspace={tree:{revision:7,nodes},statuses:[],latest_undo:undo,service_error:null};
  const bridge=fakeBridge([track("a")]);
  bridge.getPlaylists=vi.fn(async()=>JSON.parse(JSON.stringify(workspace)) as PlaylistWorkspace);
  const publish=(next:PlaylistNode[],latest_undo:string|null="operation")=>{workspace={...workspace,tree:{revision:workspace.tree.revision+1,nodes:next},latest_undo};};
  return {bridge,publish};
}
async function actions(name:string){fireEvent.click(await screen.findByRole("button",{name:`Playlist actions for ${name}`}));return screen.findByRole("menu",{name:`Actions for ${name}`});}

describe("playlist actions",()=>{
  it("edits, favorites and duplicates through revision-guarded commands without restarting playback",async()=>{
    const house=node("house","House");const {bridge,publish}=setup([house]);
    bridge.editPlaylist=vi.fn(async edit=>{
      if(edit.type==="favorite")publish([{...house,favorite_position:0}]);
      else if(edit.type==="duplicate")publish([{...house,favorite_position:0},node("copy","House",{position:1})]);
      else if(edit.type==="batch")publish([{...house,name:"Deep House"}]);
      return {edit_id:"operation",revision:8,created_ids:edit.type==="duplicate"?["copy"]:[],affected_ids:["house"]};
    });
    render(<App bridge={bridge}/>);fireEvent.doubleClick(await screen.findByText("Track a"));await screen.findByRole("button",{name:"Pause"});
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Add to favorites"}));
    await screen.findByRole("button",{name:"Favorite playlist House"});
    expect(bridge.editPlaylist).toHaveBeenLastCalledWith({type:"favorite",id:"house",position:0},7);
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Duplicate"}));
    await waitFor(()=>expect(screen.getAllByRole("button",{name:"Playlist House"})).toHaveLength(2));
    expect(bridge.editPlaylist).toHaveBeenLastCalledWith({type:"duplicate",id:"house",parent_id:null,name:null},8);
    expect(bridge.playerLoad).toHaveBeenCalledTimes(1);expect(bridge.playerPause).not.toHaveBeenCalled();
    expect(bridge.reprepareTrack).not.toHaveBeenCalled();expect(bridge.saveSettings).not.toHaveBeenCalled();
  });
  it("opens the existing editor for rename and uses its native batch",async()=>{
    const house=node("house","House");const {bridge,publish}=setup([house]);
    bridge.editPlaylist=vi.fn(async()=>{publish([{...house,name:"Deep House"}]);return {edit_id:"rename",revision:8,created_ids:[],affected_ids:["house"]};});
    render(<App bridge={bridge}/>);
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Edit"}));
    await screen.findByRole("dialog",{name:"Edit playlist"});
    fireEvent.change(screen.getByLabelText("Playlist name"),{target:{value:"Deep House"}});
    fireEvent.click(screen.getByRole("button",{name:"Save changes"}));
    await screen.findByRole("button",{name:"Playlist Deep House"});
    expect(bridge.editPlaylist).toHaveBeenCalledWith({type:"batch",edits:[{type:"rename",id:"house",name:"Deep House"}]},7);
  });
  it("removes and restores a playlist while keeping files, playback and analysis intact",async()=>{
    const house=node("house","House");const {bridge,publish}=setup([house]);
    bridge.editPlaylist=vi.fn(async edit=>{publish([{...house,deleted:edit.type==="remove",deleted_by:edit.type==="remove"?"remove":null}]);return {edit_id:"change",revision:8,created_ids:[],affected_ids:["house"]};});
    render(<App bridge={bridge}/>);fireEvent.doubleClick(await screen.findByText("Track a"));await screen.findByRole("button",{name:"Pause"});
    fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Remove"}));
    await waitFor(()=>expect(screen.queryByRole("button",{name:"Playlist House"})).not.toBeInTheDocument());
    await screen.findByRole("heading",{name:"Tracks 1"});
    fireEvent.click(screen.getByRole("button",{name:"Removed playlists"}));
    fireEvent.click(screen.getByRole("button",{name:"Restore House"}));
    await screen.findByRole("button",{name:"Playlist House"});
    expect(bridge.editPlaylist).toHaveBeenLastCalledWith({type:"restore",id:"house",parent_id:null,position:0},8);
    expect(bridge.playerLoad).toHaveBeenCalledTimes(1);expect(bridge.archiveTrack).not.toHaveBeenCalled();expect(bridge.reprepareTrack).not.toHaveBeenCalled();
  });
  it("offers durable undo after restart and keeps a rejected undo available with its error",async()=>{
    const {bridge}=setup([node("house","House")],"persisted-edit");
    bridge.undoPlaylist=vi.fn(async()=>{throw Error("Undo conflicts with later playlist edits");});
    render(<App bridge={bridge}/>);
    const undo=await screen.findByRole("button",{name:"Undo last playlist change"});await waitFor(()=>expect(undo).toBeEnabled());fireEvent.click(undo);
    await screen.findByText("Undo conflicts with later playlist edits");
    expect(bridge.undoPlaylist).toHaveBeenCalledWith("persisted-edit",7);
    await waitFor(()=>expect(undo).toBeEnabled());
    expect(screen.getByRole("button",{name:"Playlist House"})).toBeInTheDocument();
  });
  it("reorders siblings and moves to a folder without offering descendants as destinations",async()=>{
    const folder=node("sets","Sets",{kind:"folder",position:0});const child=node("child","Child",{kind:"folder",parent_id:"sets"});const house=node("house","House",{position:1});
    const {bridge,publish}=setup([folder,child,house]);
    bridge.editPlaylist=vi.fn(async edit=>{if(edit.type==="move")publish([folder,child,{...house,parent_id:edit.parent_id,position:edit.position}]);return {edit_id:"move",revision:8,created_ids:[],affected_ids:[edit.type==="move"?edit.id:""]};});
    render(<App bridge={bridge}/>);
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Move up"}));
    await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenCalledWith({type:"move",id:"house",parent_id:null,position:0},7));
    fireEvent.click(within(await actions("Sets")).getByRole("menuitem",{name:"Move to folder…"}));
    const destination=await screen.findByLabelText("Destination folder");
    expect(within(destination).queryByRole("option",{name:"Sets"})).not.toBeInTheDocument();
    expect(within(destination).queryByRole("option",{name:/Child/})).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Move to folder…"}));
    fireEvent.change(screen.getByLabelText("Destination folder"),{target:{value:"sets"}});
    fireEvent.click(screen.getByRole("button",{name:"Move"}));
    await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenLastCalledWith({type:"move",id:"house",parent_id:"sets",position:1},8));
  });
  it("completes durable undo and adopts the stored tree and next undo state",async()=>{
    const house=node("house","House");const {bridge,publish}=setup([house],"saved-edit");
    bridge.undoPlaylist=vi.fn(async()=>{publish([{...house,name:"Before rename"}],null);return {edit_id:"undo",revision:8,created_ids:[],affected_ids:["house"]};});
    render(<App bridge={bridge}/>);const undo=await screen.findByRole("button",{name:"Undo last playlist change"});await waitFor(()=>expect(undo).toBeEnabled());fireEvent.click(undo);
    await screen.findByRole("button",{name:"Playlist Before rename"});expect(bridge.undoPlaylist).toHaveBeenCalledWith("saved-edit",7);await waitFor(()=>expect(undo).toBeDisabled());
  });
  it("does not redirect a new scope when an earlier duplicate finishes",async()=>{
    const house=node("house","House"),techno=node("techno","Techno",{position:1});const {bridge,publish}=setup([house,techno],"older");
    let finish!:()=>void;
    bridge.editPlaylist=vi.fn(()=>new Promise<PlaylistChange>(resolve=>{finish=()=>{publish([house,techno,node("copy","Copy",{position:2})]);resolve({edit_id:"copy",revision:8,created_ids:["copy"],affected_ids:["copy"]});};}));
    render(<App bridge={bridge}/>);
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Duplicate"}));
    await waitFor(()=>expect(screen.getByRole("button",{name:"Undo last playlist change"})).toBeDisabled());
    fireEvent.click(screen.getByRole("button",{name:"Playlist Techno"}));await screen.findByRole("heading",{name:"Techno 1"});finish();
    await screen.findByRole("button",{name:"Playlist Copy"});expect(screen.getByRole("heading",{name:"Techno 1"})).toBeInTheDocument();expect(bridge.editPlaylist).toHaveBeenCalledTimes(1);
  });
  it("moves an internal drag into a folder and rejects a folder-to-descendant cycle",async()=>{
    const folder=node("sets","Sets",{kind:"folder"}),child=node("child","Child",{kind:"folder",parent_id:"sets"}),house=node("house","House",{position:1});const {bridge}=setup([folder,child,house]);
    render(<App bridge={bridge}/>);
    const folderButton=await screen.findByRole("button",{name:"Folder Sets"});fireEvent.click(screen.getByRole("button",{name:"Expand Sets"}));
    const row=(name:string)=>screen.getByRole("button",{name}).closest("[draggable]")!;
    pointerDrag(row("Folder Sets"),row("Folder Child"));
    expect(bridge.editPlaylist).not.toHaveBeenCalled();
    pointerDrag(row("Playlist House"),folderButton.closest("[draggable]")!);
    await waitFor(()=>expect(bridge.editPlaylist).toHaveBeenCalledWith({type:"move",id:"house",parent_id:"sets",position:1},7));
  });
  it("does not query the removed scope again after a successful removal",async()=>{
    const house=node("house","House");const {bridge,publish}=setup([house]);let removed=false;
    bridge.listTracks=vi.fn(async q=>{if(removed&&q.playlist_id==="house")throw Error("Playlist is removed: house");return {rows:[track("a")],total:1,revision:1,next_cursor:null};});
    bridge.editPlaylist=vi.fn(async()=>{removed=true;publish([{...house,deleted:true,deleted_by:"remove"}]);return {edit_id:"remove",revision:8,created_ids:[],affected_ids:["house"]};});
    render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"House 1"});
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Remove"}));
    await screen.findByRole("heading",{name:"Tracks 1"});
    expect(screen.queryByText("Playlist is removed: house")).not.toBeInTheDocument();
  });
  it("keeps Cmd+A available after selecting a playlist with its sidebar button",async()=>{
    const {bridge}=setup([node("house","House")]);bridge.selectTrackIds=vi.fn(async()=>["a","offscreen"]);
    render(<App bridge={bridge}/>);const button=await screen.findByRole("button",{name:"Playlist House"});fireEvent.click(button);await screen.findByRole("heading",{name:"House 1"});
    fireEvent.keyDown(button,{key:"a",metaKey:true});await screen.findByText("2 selected");
    expect(bridge.selectTrackIds).toHaveBeenCalledWith(expect.objectContaining({playlist_id:"house",sort:"playlist_order"}));
  });
  it("clears the playlist scope when new files enter Incoming",async()=>{
    const {bridge}=setup([node("house","House")]);bridge.chooseFiles=vi.fn(async()=>["/preview/new.mp3"]);
    bridge.enqueuePaths=vi.fn(async()=>{bridge.rows.push({...track("new"),state:"incoming"});return [];});
    render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"House 1"});
    fireEvent.click(screen.getByRole("button",{name:"Add tracks"}));await screen.findByRole("heading",{name:"Incoming 1"});
    await waitFor(()=>expect(bridge.listTracks).toHaveBeenLastCalledWith(expect.objectContaining({view:"incoming",playlist_id:null})));
  });
  it("opens Settings when navigating away from a playlist editor",async()=>{
    const {bridge}=setup([node("house","House")]);render(<App bridge={bridge}/>);
    fireEvent.click(within(await actions("House")).getByRole("menuitem",{name:"Edit"}));await screen.findByRole("dialog",{name:"Edit playlist"});
    fireEvent.click(screen.getByRole("button",{name:"Settings"}));
    await waitFor(()=>expect(screen.queryByRole("dialog",{name:"Edit playlist"})).not.toBeInTheDocument());
    await screen.findByRole("heading",{name:"Settings"});
  });
});
