import {act,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import TaggingSettings from "../views/TaggingSettings";
import catalog from "./tagging-catalog.json";
import {fakeBridge} from "./bridge";
import type {CatalogProviderId,CredentialProviderId,CredentialRef,CredentialStatus,ProviderCredentialInput,ProviderSetupStatus} from "../provider-setup-types";
import type {TaggingCatalog} from "../types";

function fixture(){
 const bridge=fakeBridge([]),base=structuredClone(catalog) as TaggingCatalog;
 const names={musicbrainz:"MusicBrainz",sonovault:"SonoVault",discogs:"Discogs",bpmsupreme:"BPM Supreme",volumo:"Volumo"};
 base.providers.push(...Object.entries(names).map(([id,name])=>({id,name,maxThreads:1,supportedTags:["title","artist","albumArt"],customOptions:{options:[]}})));
 bridge.getTaggingCatalog=vi.fn(async()=>base);
 const missing=(provider:CredentialProviderId):CredentialStatus=>({reference:{provider,generation:"g1",session:"s1"},presence:"missing",vault:"ready",verification:"untested"});
 const statuses:ProviderSetupStatus[]=Object.entries(names).map(([id])=>({id:id as CatalogProviderId,revision:"r1",setup_kind:["sonovault","volumo"].includes(id)?"api_key":id==="discogs"?"personal_token":id==="bpmsupreme"?"email_password":"none",credential:id==="musicbrainz"?null:missing(id==="bpmsupreme"?"bpm_supreme":id as CredentialProviderId),auth_mode:id==="discogs"?"anonymous":id==="musicbrainz"?"public":"unavailable",can_use:["musicbrainz","discogs"].includes(id),reason:null,rate_limit_per_minute:id==="discogs"?25:null,min_request_interval_ms:id==="musicbrainz"?1000:null,retry_at_ms:null,quota:null}));
 const native=Object.assign(bridge,{providerSetupStatuses:vi.fn(async()=>structuredClone(statuses)),saveProviderCredential:vi.fn(async(expected:CredentialRef,_input:ProviderCredentialInput):Promise<CredentialStatus>=>({reference:{...expected,generation:"g2"},presence:"present",vault:"ready",verification:"untested"})),testProviderCredential:vi.fn(async()=>statuses[1]),removeProviderCredential:vi.fn(async()=>missing("sonovault"))});
 return {bridge:native,config:base.defaults,statuses};
}
describe("native provider credentials",()=>{
 it("requires optional Volumo setup and saves a masked personal key separately from provider selection",async()=>{
  const {bridge,config}=fixture(),change=vi.fn();render(<TaggingSettings bridge={bridge} config={config} enabled onChange={change}/>);expect(await screen.findByLabelText("Use Volumo")).toBeDisabled();fireEvent.click(screen.getByRole("button",{name:"Set up Volumo"}));
  expect(screen.getByRole("link",{name:"Create a Volumo API token"})).toHaveAttribute("href","https://volumo.com/profile/api-tokens");expect(screen.getByText(/ISRC links and public track metadata/)).toBeInTheDocument();
  const key=screen.getByLabelText("Volumo API key");expect(key).toHaveAttribute("type","password");fireEvent.change(key,{target:{value:"invented-volumo-key"}});fireEvent.click(screen.getByRole("button",{name:"Save Volumo key"}));await waitFor(()=>expect(bridge.saveProviderCredential).toHaveBeenCalledWith({provider:"volumo",generation:"g1",session:"s1"},{kind:"api_key",secret:"invented-volumo-key"}));expect(key).toHaveValue("");expect(change).not.toHaveBeenCalled();expect(bridge.saveSettings).not.toHaveBeenCalled();
 });
 it("shows cancellation while a connection test is still pending and preserves saved settings",async()=>{
  const {bridge,config,statuses}=fixture(),change=vi.fn();statuses[1].credential!.presence="present";let complete!:(value:ProviderSetupStatus)=>void,requestedId:unknown;
  bridge.testProviderCredential=vi.fn((...args:unknown[])=>{requestedId=args[0];return new Promise<ProviderSetupStatus>(resolve=>{complete=resolve;});});
  render(<TaggingSettings bridge={bridge} config={config} enabled onChange={change}/>);fireEvent.click(await screen.findByRole("button",{name:"Manage SonoVault"}));fireEvent.click(screen.getByRole("button",{name:"Test saved SonoVault connection"}));await waitFor(()=>expect(bridge.testProviderCredential).toHaveBeenCalledTimes(1));
  fireEvent.click(await screen.findByRole("button",{name:"Cancel provider operation"}));expect(requestedId).toEqual(expect.any(String));expect(bridge.cancelMetadataOperation).toHaveBeenCalledWith(requestedId);expect(change).not.toHaveBeenCalled();expect(bridge.saveSettings).not.toHaveBeenCalled();
  await act(async()=>complete(statuses[1]));await waitFor(()=>expect(screen.queryByRole("button",{name:"Cancel provider operation"})).not.toBeInTheDocument());
 });
 it("ignores an older catalog refresh after a newer detected restriction without changing draft choices",async()=>{
  const {bridge,config}=fixture(),change=vi.fn();config.options.platforms=["beatport"];let refresh!:()=>void;bridge.onCatalogAccessChange=vi.fn(async handler=>{refresh=handler;return()=>{};});
  const direct=await bridge.getTaggingCatalog(),restricted=structuredClone(direct);restricted.access=[{id:"traxsource",requires_solver:true}];
  render(<TaggingSettings bridge={bridge} config={config} enabled onChange={change}/>);expect(await screen.findByLabelText("Use Traxsource")).toBeEnabled();let old!:(value:TaggingCatalog)=>void;
  bridge.getTaggingCatalog=vi.fn().mockImplementationOnce(()=>new Promise<TaggingCatalog>(resolve=>{old=resolve;})).mockResolvedValueOnce(restricted);
  act(()=>refresh());act(()=>refresh());await waitFor(()=>expect(screen.getByLabelText("Use Traxsource")).toBeDisabled());
  await act(async()=>old(direct));expect(screen.getByLabelText("Use Traxsource")).toBeDisabled();expect(change).not.toHaveBeenCalled();
 });
 it("keeps Remove available after uncertain deletion and never displays raw native error contents",async()=>{
  const {bridge,config,statuses}=fixture();statuses[1].credential!.presence="present";bridge.removeProviderCredential=vi.fn(async()=>{statuses[1].credential!.presence="unknown";statuses[1].credential!.vault="uncertain";statuses[1].credential!.reference.generation="uncertain-removal";throw Error("fixture-sensitive-native-error");});
  render(<TaggingSettings bridge={bridge} config={config} enabled onChange={()=>{}}/>);fireEvent.click(await screen.findByRole("button",{name:"Manage SonoVault"}));fireEvent.click(screen.getByRole("button",{name:"Remove SonoVault key"}));
  expect(await screen.findByRole("alert")).not.toHaveTextContent("fixture-sensitive-native-error");await screen.findByText("Credential change needs attention",{selector:"p"});expect(screen.getByRole("button",{name:"Remove SonoVault key"})).toBeEnabled();expect(screen.queryByText("Removed")).not.toBeInTheDocument();
 });
 it("refreshes access-only status without discarding typed replacement or unsaved solver settings",async()=>{
  const {bridge,config,statuses}=fixture(),change=vi.fn();let refresh!:()=>void;bridge.onCatalogAccessChange=vi.fn(async handler=>{refresh=handler;return()=>{};});
  const view=render(<TaggingSettings bridge={bridge} config={config} enabled onChange={change}/>);fireEvent.click(await screen.findByRole("button",{name:"Set up SonoVault"}));fireEvent.change(screen.getByLabelText("SonoVault API key"),{target:{value:"unsent-private-value"}});
  fireEvent.change(screen.getByLabelText("Solver API URL"),{target:{value:"http://solver.invalid:8191"}});const draft=change.mock.calls.at(-1)![0];view.rerender(<TaggingSettings bridge={bridge} config={draft} enabled onChange={change}/>);
  statuses[1].credential!.presence="present";statuses[1].credential!.verification="email_unverified";statuses[1].credential!.reference.generation="external-key";act(()=>refresh());
  await screen.findByText("Verify your email before use",{selector:"p"});expect(screen.getByLabelText("SonoVault API key")).toHaveValue("unsent-private-value");expect(screen.getByLabelText("Solver API URL")).toHaveValue("http://solver.invalid:8191");expect(bridge.saveProviderCredential).not.toHaveBeenCalled();expect(bridge.saveSettings).not.toHaveBeenCalled();
 });
 it("lets Remove revoke a pending Test and ignores its late valid result",async()=>{
  const {bridge,config,statuses}=fixture();statuses[1].credential!.presence="present";statuses[1].can_use=true;let complete!:(value:typeof statuses[number])=>void;
  bridge.testProviderCredential=vi.fn(()=>new Promise<ProviderSetupStatus>(resolve=>{complete=resolve;}));bridge.removeProviderCredential=vi.fn(async()=>{statuses[1].credential!.presence="missing";statuses[1].credential!.verification="untested";statuses[1].credential!.reference.generation="removed";statuses[1].can_use=false;return statuses[1].credential!;});
  render(<TaggingSettings bridge={bridge} config={config} enabled onChange={()=>{}}/>);fireEvent.click(await screen.findByRole("button",{name:"Manage SonoVault"}));fireEvent.click(screen.getByRole("button",{name:"Test saved SonoVault connection"}));await waitFor(()=>expect(bridge.testProviderCredential).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button",{name:"Remove SonoVault key"}));await waitFor(()=>expect(screen.getByLabelText("Use SonoVault")).toBeDisabled());
  const valid=structuredClone(statuses[1]);valid.credential!.presence="present";valid.credential!.verification="valid";valid.can_use=true;await act(async()=>complete(valid));expect(screen.getByLabelText("Use SonoVault")).toBeDisabled();expect(screen.queryByText("Connection verified")).not.toBeInTheDocument();expect(bridge.cancelMetadataOperation).toHaveBeenCalled();
 });
 it("keeps MusicBrainz and anonymous Discogs available while only optional key/account providers need setup",async()=>{
  const {bridge,config}=fixture();render(<TaggingSettings bridge={bridge} config={config} enabled onChange={()=>{}}/>);
  expect(await screen.findByLabelText("Use MusicBrainz")).toBeEnabled();expect(screen.getByLabelText("Use Discogs")).toBeEnabled();
  await waitFor(()=>expect(screen.getByLabelText("Use SonoVault")).toBeDisabled());
  expect(screen.getByText(/Public metadata · no account required/)).toBeInTheDocument();expect(screen.getByText(/Anonymous · 25 requests\/minute/)).toBeInTheDocument();
  expect(screen.queryByLabelText("Use Spotify")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Use Beatsource")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Use Juno Download")).not.toBeInTheDocument();
 });
 it("saves a transient masked SonoVault key natively without enabling providers or saving global settings",async()=>{
  const {bridge,config}=fixture(),change=vi.fn();render(<TaggingSettings bridge={bridge} config={config} enabled onChange={change}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Set up SonoVault"}));
  const input=screen.getByLabelText("SonoVault API key");expect(input).toHaveAttribute("type","password");fireEvent.change(input,{target:{value:"fixture-secret"}});
  fireEvent.click(screen.getByRole("button",{name:"Save SonoVault key"}));
  await waitFor(()=>expect(bridge.saveProviderCredential).toHaveBeenCalledWith({provider:"sonovault",generation:"g1",session:"s1"},{kind:"api_key",secret:"fixture-secret"}));
  expect(input).toHaveValue("");expect(change).not.toHaveBeenCalled();expect(bridge.saveSettings).not.toHaveBeenCalled();expect(screen.getByRole("link",{name:"Get a free API key"})).toHaveAttribute("href","https://sonovault.now/signup");
  expect(JSON.stringify(config)).not.toContain("fixture-secret");expect(screen.queryByText("fixture-secret")).not.toBeInTheDocument();
 });
 it("uses a native email/password account action for BPM Supreme and never offers Spotify token paste",async()=>{
  const {bridge,config}=fixture();render(<TaggingSettings bridge={bridge} config={config} enabled onChange={()=>{}}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Set up BPM Supreme"}));const setup=screen.getByRole("region",{name:"BPM Supreme setup"});
  fireEvent.change(within(setup).getByLabelText("BPM Supreme email"),{target:{value:"dj@example.invalid"}});fireEvent.change(within(setup).getByLabelText("BPM Supreme password"),{target:{value:"fixture password"}});
  fireEvent.click(within(setup).getByRole("button",{name:"Save BPM Supreme account"}));
  await waitFor(()=>expect(bridge.saveProviderCredential).toHaveBeenCalledWith(expect.objectContaining({provider:"bpm_supreme"}),{kind:"email_password",email:"dj@example.invalid",password:"fixture password"}));
  expect(screen.queryByLabelText(/Spotify.*token/i)).not.toBeInTheDocument();
 });
});
