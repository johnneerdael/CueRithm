import {describe,it,expect,vi} from "vitest";
const native=vi.hoisted(()=>({invoke:vi.fn(async()=>undefined),listen:vi.fn(async()=>()=>{}),close:vi.fn(async()=>{}),onCloseRequested:vi.fn()}));
vi.mock("@tauri-apps/api/core",()=>({invoke:native.invoke}));
vi.mock("@tauri-apps/api/event",()=>({listen:native.listen}));
vi.mock("@tauri-apps/api/webviewWindow",()=>({getCurrentWebviewWindow:()=>({close:native.close,onCloseRequested:native.onCloseRequested})}));
import {nativeBridge} from "../api";
describe("native inspector boundary",()=>{
 it("binds review apply to the native previewId argument",async()=>{
  await nativeBridge.applyMatchReview("operation","preview","revision");
  expect(native.invoke).toHaveBeenLastCalledWith("apply_match_review",{operationId:"operation",previewId:"preview",revision:"revision"});
 });
 it("waits for durable drafts before closing and does not repeat the close guard",async()=>{
  let listener!:(event:{preventDefault:()=>void})=>Promise<void>;native.onCloseRequested.mockImplementation(async call=>{listener=call;return()=>{};});
  let finish!:()=>void;const durable=new Promise<void>(resolve=>{finish=resolve;});
  const failure=vi.fn();await nativeBridge.onCloseRequest(()=>durable,failure);
  const event={preventDefault:vi.fn()},allowed={preventDefault:vi.fn()};
  native.close.mockImplementationOnce(async()=>{await listener(allowed);});
  const closing=listener(event);expect(event.preventDefault).toHaveBeenCalledTimes(1);expect(native.close).not.toHaveBeenCalled();
  finish();await closing;expect(native.close).toHaveBeenCalledTimes(1);expect(allowed.preventDefault).not.toHaveBeenCalled();expect(failure).not.toHaveBeenCalled();
 });
 it("keeps the window open if local draft storage fails",async()=>{
  native.close.mockClear();let listener!:(event:{preventDefault:()=>void})=>Promise<void>;native.onCloseRequested.mockImplementation(async call=>{listener=call;return()=>{};});
  const failure=vi.fn();await nativeBridge.onCloseRequest(async()=>{throw Error("Draft disk is full");},failure);
  await listener({preventDefault:vi.fn()});expect(native.close).not.toHaveBeenCalled();expect(failure).toHaveBeenCalledWith(expect.objectContaining({message:"Draft disk is full"}));
 });
});
