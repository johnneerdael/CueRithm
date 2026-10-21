import {render,screen,fireEvent,within,waitFor} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge} from "./bridge";
const open=async()=>{const bridge=fakeBridge([]);render(<App bridge={bridge}/>);await screen.findByText("Your library starts here");fireEvent.click(screen.getByRole("button",{name:"Settings"}));fireEvent.click(within(screen.getByRole("navigation",{name:"Settings pages"})).getByRole("button",{name:"File organization"}));return bridge;};
it("defaults to leaving files in place and saves native-validated organization drafts with other settings",async()=>{
 const bridge=await open();expect(screen.getByLabelText("Organization mode")).toHaveValue("leave_in_place");expect(screen.queryByLabelText("Filename pattern")).not.toBeInTheDocument();
 fireEvent.change(screen.getByLabelText("Organization mode"),{target:{value:"rename_in_place"}});
 await screen.findByText("Artist - Title.flac",{exact:false});
 fireEvent.change(screen.getByLabelText("Filename pattern"),{target:{value:"{title}[ ({version})]"}});
 await waitFor(()=>expect(bridge.organizationExample).toHaveBeenLastCalledWith(expect.objectContaining({pattern:"{title}[ ({version})]"})));
 const nav=screen.getByRole("navigation",{name:"Settings pages"});fireEvent.click(within(nav).getByRole("button",{name:"Cue generation"}));fireEvent.change(screen.getByLabelText("Minimum breakdown length"),{target:{value:"128"}});fireEvent.click(within(nav).getByRole("button",{name:"File organization"}));
 expect(screen.getByLabelText("Filename pattern")).toHaveValue("{title}[ ({version})]");
 await waitFor(()=>expect(screen.getByRole("button",{name:"Save settings"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Save settings"}));await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
 expect(vi.mocked(bridge.saveSettings).mock.calls[0][0]).toEqual(expect.objectContaining({engine:expect.objectContaining({lexicon_config:expect.objectContaining({min_cue_length_beats:128})}),organization:expect.objectContaining({mode:"rename_in_place",pattern:"{title}[ ({version})]"})}));
});
it("saves only an opaque destination ID and blocks invalid native naming rules",async()=>{
 const bridge=await open();vi.mocked(bridge.chooseOrganizationRoot).mockResolvedValue({id:"native-root",display_path:"/Invented/Music"});
 fireEvent.change(screen.getByLabelText("Organization mode"),{target:{value:"move"}});fireEvent.click(screen.getByRole("button",{name:"Choose destination"}));await screen.findByText("/Invented/Music");
 await waitFor(()=>expect(screen.getByRole("button",{name:"Save settings"})).toBeEnabled());
 fireEvent.click(screen.getByRole("button",{name:"Save settings"}));await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());expect(vi.mocked(bridge.saveSettings).mock.calls[0][0].organization?.destination_root_id).toBe("native-root");expect(JSON.stringify(vi.mocked(bridge.saveSettings).mock.calls[0][0])).not.toContain("/Invented/Music");
 await screen.findByRole("button",{name:"Settings saved"});
 vi.mocked(bridge.organizationExample).mockResolvedValue({valid:false,error:"Unknown naming token",tokens:[],examples:[]});fireEvent.change(screen.getByLabelText("Filename pattern"),{target:{value:"{unknown}"}});await screen.findByRole("alert");expect(screen.getByRole("button",{name:"Save settings"})).toBeDisabled();
});
it("never offers overwrite and allows collision skip only for optional organization",async()=>{
 await open();fireEvent.change(screen.getByLabelText("Organization mode"),{target:{value:"rename_in_place"}});await screen.findByText("Artist - Title.flac",{exact:false});
 const policy=screen.getByLabelText("Organization collision policy");expect(within(policy).getByRole("option",{name:"Skip track (optional organization)"})).toBeDisabled();expect(screen.queryByRole("option",{name:/overwrite/i})).not.toBeInTheDocument();
 fireEvent.click(screen.getByLabelText("Organization is optional"));expect(within(policy).getByRole("option",{name:"Skip track (optional organization)"})).toBeEnabled();fireEvent.change(policy,{target:{value:"skip"}});fireEvent.click(screen.getByLabelText("Organization is optional"));expect(policy).toHaveValue("block");
});
