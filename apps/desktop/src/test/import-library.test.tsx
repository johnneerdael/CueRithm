import {describe,it,expect,vi} from "vitest";
import {render,screen} from "@testing-library/react";
import {ImportSelection} from "../views/ImportSelection";
import {importInspection,importTrack} from "./import-fixtures";
import App from "../App";
import PlayerPanel from "../components/PlayerPanel";
import {fakeBridge,track} from "./bridge";
import userEvent from "@testing-library/user-event";
import {importApi} from "../import-api";
describe("imported library observations",()=>{
 it("opens persisted imported playlists from the desktop sidebar",async()=>{
  const listing=vi.spyOn(importApi,"importedCollectionList").mockResolvedValue({records:[{id:"native-list",source_id:"source",provider:"m3u",parent_id:null,name:"Persisted imported set",name_truncated:false,kind:"playlist",occurrences:3}],total:1,next_cursor:null});
  try{render(<App bridge={fakeBridge([])}/>);await userEvent.click(await screen.findByRole("button",{name:"Imported playlists"}));expect(await screen.findByText("Persisted imported set")).toBeVisible();}finally{listing.mockRestore();}
 });
 it("missing_media_stays_in_library",()=>{
  render(<ImportSelection inspection={importInspection} page={{records:[{record_type:"track",record:importTrack}],total:1,next_cursor:null}} kind="track" selected={{selection:"all"}} onSelection={()=>{}} onKind={()=>{}} onNext={()=>{}} busy={false}/>);
  expect(screen.getByText("Imported title")).toBeVisible();expect(screen.getByText(/^Missing media \//i)).toBeVisible();expect(screen.getByText("8A")).toBeVisible();expect(screen.queryByText(/prepared/i)).not.toBeInTheDocument();
 });
 it("unavailable imported tracks cannot start playback",async()=>{
  const row={...track("missing"),state:"imported",import_state:{source_ids:["source"],availability:"missing",analysis_state:"unanalysed",playback_allowed:false}};
  const bridge=fakeBridge([row]);render(<App bridge={bridge}/>);
  expect(await screen.findByRole("button",{name:"Play Track missing"})).toBeDisabled();
  expect(bridge.playerLoad).not.toHaveBeenCalled();
 });
 it("the player shows accepted imported fields without prepared analysis",async()=>{
  const row={...track("source"),state:"imported",import_state:{source_ids:["source"],availability:"available",analysis_state:"unanalysed",playback_allowed:true}};
  const bridge=fakeBridge([row]);const status=await bridge.playerStatus();
  const {container}=render(<PlayerPanel status={status} detail={{track:row,analysis:null,source_sha256:row.source_sha256,pcm_sha256:null}} data={null} busy={false} onToggle={vi.fn()} onPause={vi.fn()} onSeek={vi.fn()} onVolume={vi.fn()}/>);
  expect(container.querySelector(".player-key")).toHaveTextContent("8A");
  expect(container.querySelector(".player-analysis")).toHaveTextContent("128.00");
  expect(container.querySelector(".player-analysis")).toHaveTextContent("6");
 });
});
