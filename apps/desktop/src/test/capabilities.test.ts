import {describe,it,expect} from "vitest";
import capabilityText from "../../src-tauri/capabilities/main.json?raw";
// `core:default` only lets a window be read. Each window call the frontend makes needs its own permission,
// and a call without one is rejected with "not allowed by ACL" at run time, which a mocked test cannot see:
// closing the window with the red button saved the drafts and then failed on `window.close()`, and after that on the
// `destroy()` that Tauri's own `onCloseRequested` wrapper makes once the handler lets the close go ahead.
const WINDOW_PERMISSIONS:Record<string,string>={
  "window.close(":"core:window:allow-close",
  "onCloseRequested(":"core:window:allow-destroy",
  "window.destroy(":"core:window:allow-destroy",
  "window.hide(":"core:window:allow-hide",
  "window.minimize(":"core:window:allow-minimize",
  "window.setFocus(":"core:window:allow-set-focus",
  "window.setTitle(":"core:window:allow-set-title",
};
const files=import.meta.glob<string>(["../**/*.{ts,tsx}","!../test/**"],{query:"?raw",import:"default",eager:true});
describe("main window capability",()=>{
  const capability=JSON.parse(capabilityText) as {permissions:Array<string|{identifier:string}>};
  const granted=new Set(capability.permissions.map(entry=>typeof entry==="string"?entry:entry.identifier));
  const code=Object.values(files).join("\n");
  it("grants every window permission the frontend calls",()=>{
    const missing=Object.entries(WINDOW_PERMISSIONS).filter(([call,permission])=>code.includes(call)&&!granted.has(permission));
    expect(missing).toEqual([]);
  });
  it("still has a window call to guard (the table above is not stale)",()=>{
    expect(Object.keys(files).length).toBeGreaterThan(20);
    expect(code).toContain("window.close(");
    expect(code).toContain("onCloseRequested(");
    expect(granted.has("core:window:allow-close")).toBe(true);
    expect(granted.has("core:window:allow-destroy")).toBe(true);
  });
});
