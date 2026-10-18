import {render,screen,fireEvent} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";
import {workspace,playlistNode} from "./playlist-fixtures";
describe("smart playlist status",()=>{
 it("shows the evaluation error and last-good result without offering manual membership edits",async()=>{
  const bridge=fakeBridge([track("a")]);const state=workspace();state.tree.nodes.push(playlistNode("smart","Smart","smartlist"));state.statuses=[{playlist_id:"smart",state:"error",error:"Measured waveform is unavailable",evaluated_at:1,next_refresh:null,last_good_revision:3}];bridge.getPlaylists=vi.fn(async()=>state);
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Smart playlist Smart"}));await screen.findByText("Measured waveform is unavailable");expect(screen.getByText("Showing the last successfully evaluated results.")).toBeInTheDocument();fireEvent.click(await screen.findByText("Track a"));expect(screen.queryByRole("button",{name:"Remove from playlist"})).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:"Move selected tracks up"})).not.toBeInTheDocument();
 });
 it("shows an empty playlist instead of suggesting that the library has no music",async()=>{
  const bridge=fakeBridge([]);bridge.getPlaylists=vi.fn(async()=>workspace());render(<App bridge={bridge}/>);fireEvent.click(await screen.findByRole("button",{name:"Playlist House"}));await screen.findByRole("heading",{name:"No tracks in this playlist"});expect(screen.queryByRole("heading",{name:"Your library starts here"})).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:"Add audio files"})).not.toBeInTheDocument();
 });
});
