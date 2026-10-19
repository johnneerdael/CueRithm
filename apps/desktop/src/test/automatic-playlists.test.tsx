import {render,screen,fireEvent,within,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge} from "./bridge";
async function open(){fireEvent.click(await screen.findByRole("button",{name:"Settings"}));fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Smart Playlists"}));await screen.findByRole("checkbox",{name:"By Artist"});}
describe("automatic smart playlists",()=>{
 it("requires opt-in and saves all five options without changing preparation",async()=>{
  const bridge=fakeBridge([]);render(<App bridge={bridge}/>);await open();
  for(const name of ["By Artist","By BPM","By release date","By Genre","By Key"])expect(screen.getByRole("checkbox",{name})).not.toBeChecked();
  fireEvent.click(screen.getByRole("checkbox",{name:"By Artist"}));fireEvent.click(screen.getByRole("checkbox",{name:"By Key"}));
  fireEvent.click(screen.getByRole("button",{name:"Save Smart Playlist settings"}));
  await screen.findByText("Smart Playlist settings saved.");
  expect(bridge.savePlaylistPreferences).toHaveBeenCalledWith(expect.objectContaining({automatic:expect.objectContaining({artists:true,keys:true,bpm:false,release_dates:false,genres:false})}),expect.any(Number));
  expect(bridge.saveSettings).not.toHaveBeenCalled();
 });
 it("validates inclusive ranges, caps them at ten and retains failed saves",async()=>{
  const bridge=fakeBridge([]);bridge.savePlaylistPreferences=vi.fn(async()=>{throw Error("Preferences changed");});render(<App bridge={bridge}/>);await open();
  fireEvent.click(screen.getByRole("checkbox",{name:"By BPM"}));
  for(let n=0;n<10;n++)fireEvent.click(screen.getByRole("button",{name:"Add BPM range"}));
  expect(screen.getByRole("button",{name:"Add BPM range"})).toBeDisabled();
  fireEvent.change(screen.getByRole("spinbutton",{name:"Range 1 minimum BPM"}),{target:{value:"150"}});
  fireEvent.change(screen.getByRole("spinbutton",{name:"Range 1 maximum BPM"}),{target:{value:"120"}});
  expect(screen.getByRole("button",{name:"Save Smart Playlist settings"})).toBeDisabled();
  for(let n=10;n>=2;n--)fireEvent.click(screen.getByRole("button",{name:`Remove BPM range ${n}`}));
  fireEvent.change(screen.getByRole("spinbutton",{name:"Range 1 maximum BPM"}),{target:{value:"150"}});
  fireEvent.click(screen.getByRole("button",{name:"Save Smart Playlist settings"}));
  await screen.findByText("Preferences changed");expect(screen.getByRole("spinbutton",{name:"Range 1 minimum BPM"})).toHaveValue(150);
  expect(bridge.savePlaylistPreferences).toHaveBeenCalledWith(expect.objectContaining({automatic:expect.objectContaining({bpm:true,bpm_ranges:[{start:150,end:150}]})}),expect.any(Number));
  fireEvent.click(screen.getByRole("button",{name:"Reload library preferences"}));await waitFor(()=>expect(screen.getByRole("checkbox",{name:"By BPM"})).not.toBeChecked());
 });
});
