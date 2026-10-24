import {describe,it,expect,vi} from "vitest";
import {render,screen,waitFor} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {ImportedCollections} from "../views/ImportedCollections";
import {importFixtureApi} from "./import-fixtures";
import {track} from "./bridge";
describe("imported playlists",()=>{
 it("opens persisted playlists and preserves repeated occurrences and missing entries",async()=>{
  const api={...importFixtureApi(),importedCollectionList:vi.fn(async()=>({records:[{id:"playlist-local",source_id:"source",provider:"m3u" as const,parent_id:null,name:"Imported set",name_truncated:false,kind:"playlist",occurrences:3}],total:1,next_cursor:null}))};
  const a={...track("a"),title:"Repeated track",state:"imported" as const,import_state:{source_ids:["source"],availability:"available",analysis_state:"unanalysed",playback_allowed:true}};
  api.importedCollectionPage=vi.fn(async()=>({id:"playlist-local",ordered:true,kind:"playlist",rows:[{occurrence_id:"one",item_id:"a",source_track_id:"native-a",source_position:0,position:0,track:a},{occurrence_id:"two",item_id:null,source_track_id:"missing-b",source_position:1,position:1,track:null},{occurrence_id:"three",item_id:"a",source_track_id:"native-a",source_position:2,position:2,track:a}],total:3,next_cursor:null}));
  const preview=vi.fn();render(<ImportedCollections api={api} onPreview={preview}/>);
  await userEvent.click(await screen.findByRole("button",{name:"Open Imported set"}));
  expect(await screen.findAllByText("Repeated track")).toHaveLength(2);
  expect(screen.getByText("Unresolved source track: missing-b")).toBeVisible();
  expect(screen.getAllByRole("button",{name:"Preview Repeated track"})).toHaveLength(2);
  await userEvent.click(screen.getAllByRole("button",{name:"Preview Repeated track"})[1]);
  expect(preview).toHaveBeenCalledWith(a);
  expect(screen.queryByText(/prepared/i)).not.toBeInTheDocument();
 });
 it("pages folders by server IDs and reloads after a stale cursor",async()=>{
  const api={...importFixtureApi(),importedCollectionList:vi.fn(async(query:{parent_id?:string|null;cursor?:string|null}={})=>{if(query.cursor)throw {code:"stale_cursor",message:"Collection list changed; reload"};return {records:query.parent_id?[]:[{id:"folder",source_id:"source",provider:"traktor" as const,parent_id:null,name:"Native folder",name_truncated:false,kind:"folder",occurrences:0}],total:1,next_cursor:query.parent_id?null:"server-next"};})};
  render(<ImportedCollections api={api} onPreview={()=>{}}/>);
  await userEvent.click(await screen.findByRole("button",{name:"Next collections page"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("Collection list changed");
  await userEvent.click(screen.getByRole("button",{name:"Reload imported collections"}));
  await userEvent.click(await screen.findByRole("button",{name:"Open Native folder"}));
  await waitFor(()=>expect(api.importedCollectionList).toHaveBeenLastCalledWith(expect.objectContaining({parent_id:"folder",source_id:"source"})));
  expect(await screen.findByText("No imported collections here.")).toBeVisible();
 });
});
