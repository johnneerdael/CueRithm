import {describe,it,expect,vi} from "vitest";
import {render,screen} from "@testing-library/react";
import {ImportSelection} from "../views/ImportSelection";
import {importInspection,importTrack} from "./import-fixtures";
import App from "../App";
import PlayerPanel from "../components/PlayerPanel";
import {fakeBridge,track} from "./bridge";
describe("imported library observations",()=>{
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
