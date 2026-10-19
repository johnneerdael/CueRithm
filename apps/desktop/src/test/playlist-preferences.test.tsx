import {render,screen,fireEvent,waitFor,within} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge} from "./bridge";
async function open(){fireEvent.click(await screen.findByRole("button",{name:"Settings"}));fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Library preferences"}));}
describe("library preferences",()=>{
 it("keeps drafts across settings pages and saves separately from preparation",async()=>{
  const bridge=fakeBridge([]);bridge.getPlaylistPreferences=vi.fn(async()=>({revision:4,preferences:{time_zone:"UTC",key_mixing_mode:"harmonic" as const}}));
  render(<App bridge={bridge}/>);await open();
  await screen.findByDisplayValue("UTC");fireEvent.change(screen.getByLabelText("Library time zone"),{target:{value:"Europe/Amsterdam"}});fireEvent.change(screen.getByLabelText("Key compatibility mode"),{target:{value:"fuzzy"}});
  fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Incoming"}));
  fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"Library preferences"}));
  expect(screen.getByLabelText("Library time zone")).toHaveValue("Europe/Amsterdam");expect(screen.queryByRole("button",{name:"Save settings"})).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Save library preferences"}));await screen.findByText("Library preferences saved.");
  expect(bridge.savePlaylistPreferences).toHaveBeenCalledWith({time_zone:"Europe/Amsterdam",key_mixing_mode:"fuzzy"},4);expect(bridge.saveSettings).not.toHaveBeenCalled();expect(bridge.reprepareTrack).not.toHaveBeenCalled();
 });
 it("keeps rejected drafts and can reload the newer native preference revision",async()=>{
  const bridge=fakeBridge([]);let revision=4;bridge.getPlaylistPreferences=vi.fn(async()=>({revision,preferences:{time_zone:"UTC",key_mixing_mode:"harmonic" as const}}));bridge.savePlaylistPreferences=vi.fn(async()=>{throw Error("Library preferences changed; reload before saving");});
  render(<App bridge={bridge}/>);await open();await screen.findByDisplayValue("UTC");fireEvent.change(screen.getByLabelText("Library time zone"),{target:{value:"Europe/London"}});fireEvent.click(screen.getByRole("button",{name:"Save library preferences"}));await screen.findByText("Library preferences changed; reload before saving");expect(screen.getByLabelText("Library time zone")).toHaveValue("Europe/London");
  revision=5;fireEvent.click(screen.getByRole("button",{name:"Reload library preferences"}));await waitFor(()=>expect(screen.getByLabelText("Library time zone")).toHaveValue("UTC"));
  fireEvent.change(screen.getByLabelText("Library time zone"),{target:{value:""}});fireEvent.click(screen.getByRole("button",{name:"Save library preferences"}));await waitFor(()=>expect(bridge.savePlaylistPreferences).toHaveBeenLastCalledWith({time_zone:null,key_mixing_mode:"harmonic"},5));
 });
});
