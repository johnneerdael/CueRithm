import {useState} from "react";
import {fireEvent,render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import SyncTargetSetup from "../components/SyncTargetSetup";
import type {TargetEnvironment} from "../sync-target-types";

describe("Windows library setup",()=>{
 it("starts Native and keeps explicit Wine drive mappings separate from host paths",async()=>{
  const changed=vi.fn(),pick=vi.fn(async()=>"/games/prefix");
  function Setup(){const [value,setValue]=useState<TargetEnvironment>({kind:"native"});return <SyncTargetSetup value={value} onChange={next=>{setValue(next);changed(next);}} chooseFolder={pick}/>;}
  render(<Setup/>);
  expect(screen.queryByRole("textbox",{name:"Wine or Proton prefix"})).not.toBeInTheDocument();
  fireEvent.change(screen.getByRole("combobox",{name:"Application environment"}),{target:{value:"windows_compatibility"}});
  fireEvent.click(screen.getByRole("button",{name:"Choose Wine or Proton prefix"}));
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"Wine or Proton prefix"})).toHaveValue("/games/prefix"));
  fireEvent.click(screen.getByRole("button",{name:"Add drive mapping"}));
  fireEvent.change(screen.getByRole("textbox",{name:"Windows root 1"}),{target:{value:"D:\\Music"}});
  fireEvent.change(screen.getByRole("textbox",{name:"Host folder 1"}),{target:{value:"/mnt/music"}});
  expect(changed.mock.lastCall?.[0]).toMatchObject({kind:"windows_compatibility",prefix:"/games/prefix",mappings:[{windows_root:"D:\\Music",host_root:"/mnt/music"}]});
  fireEvent.change(screen.getByRole("combobox",{name:"Application environment"}),{target:{value:"native"}});
  expect(changed.mock.lastCall?.[0]).toEqual({kind:"native"});
 });
 it("reports folder picker failures and keeps setup unchanged",async()=>{
  const changed=vi.fn();
  render(<SyncTargetSetup value={{kind:"windows_compatibility",prefix:"/old",installation:null,mappings:[]}} onChange={changed} chooseFolder={async()=>{throw new Error("Folder unavailable");}}/>);
  fireEvent.click(screen.getByRole("button",{name:"Choose Wine or Proton prefix"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("Folder unavailable");
  expect(changed).not.toHaveBeenCalled();
 });
});
