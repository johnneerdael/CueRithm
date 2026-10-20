import {act,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import App from "../App";
import {editorBridge} from "./editor-fixtures";
describe("application exit draft handshake",()=>{
 it("installs its listener before registering the guard",async()=>{
  const {bridge}=editorBridge();let installed!:(dispose:()=>void)=>void;
  bridge.onAppExitRequested=vi.fn(()=>new Promise<()=>void>(resolve=>{installed=resolve;}));
  render(<App bridge={bridge}/>);await screen.findByText("Track a");expect(bridge.registerExitGuard).not.toHaveBeenCalled();
  await act(async()=>{installed(()=>{});});await waitFor(()=>expect(bridge.registerExitGuard).toHaveBeenCalledTimes(1));
 });
 it("deduplicates Quit, disables new edits and flushes the latest draft before completing exit",async()=>{
  const {bridge}=editorBridge();let exit:(id:string)=>void=()=>{};
  bridge.onAppExitRequested=vi.fn(async handler=>{exit=handler;return()=>{};});
  const update=bridge.updateMetadataDraft;let release!:()=>void;const pending=new Promise<void>(resolve=>{release=resolve;});
  bridge.updateMetadataDraft=vi.fn(async(...args:Parameters<typeof update>)=>{await pending;return update(...args);});
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByText("Track a"));fireEvent.click(screen.getByRole("button",{name:"Edit metadata"}));
  fireEvent.change(await screen.findByRole("combobox",{name:"Title action"}),{target:{value:"set"}});fireEvent.change(screen.getByRole("textbox",{name:"Title"}),{target:{value:"Final native draft"}});
  act(()=>{exit("quit");exit("quit");});expect(bridge.completeExit).not.toHaveBeenCalled();expect(screen.getByRole("textbox",{name:"Title"})).toBeDisabled();
  await act(async()=>{release();});await waitFor(()=>expect(bridge.completeExit).toHaveBeenCalledExactlyOnceWith("quit"));expect(bridge.cancelExit).not.toHaveBeenCalled();
 });
 it("cancels Quit and preserves the editor if its local draft cannot be persisted",async()=>{
  const {bridge}=editorBridge();let exit:(id:string)=>void=()=>{};
  bridge.onAppExitRequested=vi.fn(async handler=>{exit=handler;return()=>{};});bridge.updateMetadataDraft=vi.fn(async()=>{throw Error("Draft storage is full");});
  render(<App bridge={bridge}/>);fireEvent.click(await screen.findByText("Track a"));fireEvent.click(screen.getByRole("button",{name:"Edit metadata"}));fireEvent.change(await screen.findByRole("combobox",{name:"Title action"}),{target:{value:"set"}});
  await screen.findByText("Draft storage is full");act(()=>exit("quit-failed"));await waitFor(()=>expect(bridge.cancelExit).toHaveBeenCalledWith("quit-failed"));expect(bridge.completeExit).not.toHaveBeenCalled();expect(screen.getByRole("heading",{name:"Edit metadata"})).toBeInTheDocument();
 });
});
