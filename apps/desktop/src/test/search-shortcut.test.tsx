import {act,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,describe,expect,it,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";

const platform=(name:string)=>vi.spyOn(window.navigator,"platform","get").mockReturnValue(name);
afterEach(()=>{vi.restoreAllMocks();document.body.querySelectorAll("[data-test-overlay]").forEach(node=>node.remove());});
// Sends a keydown the way the webview does: from the focused element, bubbling up to the window.
const press=(init:KeyboardEventInit,target:EventTarget=window)=>{
  const event=new KeyboardEvent("keydown",{...init,bubbles:true,cancelable:true});
  target.dispatchEvent(event);
  return event;
};
async function open(){
  render(<App bridge={fakeBridge([track("a")])}/>);
  await screen.findByText("Track a");
  return screen.getByRole("searchbox",{name:"Search library"});
}

describe("search shortcut",()=>{
  it("focuses the search box with Ctrl+F off macOS, whatever the letter case",async()=>{
    platform("Win32");
    const search=await open();
    for(const key of ["f","F"]){
      search.blur();
      const event=press({key,ctrlKey:true});
      expect(event.defaultPrevented,key).toBe(true);
      expect(search,key).toHaveFocus();
    }
  });
  it("focuses the search box with Cmd+F on macOS",async()=>{
    platform("MacIntel");
    const search=await open();
    const event=press({key:"f",metaKey:true});
    expect(event.defaultPrevented).toBe(true);
    expect(search).toHaveFocus();
  });
  it("leaves a bare Ctrl+F on macOS to the text field, and Cmd+F off macOS to the system",async()=>{
    platform("MacIntel");
    const search=await open();
    for(const init of [{key:"f",ctrlKey:true},{key:"f",metaKey:true,ctrlKey:true}]){
      const event=press(init);
      expect(event.defaultPrevented,JSON.stringify(init)).toBe(false);
      expect(search,JSON.stringify(init)).not.toHaveFocus();
    }
    platform("Linux x86_64");
    const event=press({key:"f",metaKey:true});
    expect(event.defaultPrevented).toBe(false);
    expect(search).not.toHaveFocus();
  });
  it("ignores the shortcut with Shift or Alt held, and a bare letter",async()=>{
    platform("Win32");
    const search=await open();
    for(const init of [{key:"F",ctrlKey:true,shiftKey:true},{key:"f",ctrlKey:true,altKey:true},{key:"f"}]){
      const event=press(init);
      expect(event.defaultPrevented,JSON.stringify(init)).toBe(false);
      expect(search,JSON.stringify(init)).not.toHaveFocus();
    }
  });
  it.each(["dialog","menu"])("does not pull focus out of an open %s",async role=>{
    platform("Win32");
    const search=await open();
    const overlay=document.createElement("div");
    overlay.setAttribute("role",role);overlay.setAttribute("data-test-overlay","");
    overlay.innerHTML="<button>Inside</button>";
    document.body.append(overlay);
    const inside=overlay.querySelector("button")!;
    inside.focus();
    const event=press({key:"f",ctrlKey:true},inside);
    expect(event.defaultPrevented).toBe(false);
    expect(search).not.toHaveFocus();
    expect(inside).toHaveFocus();
  });
  it("still works from a tool page, where it returns to the library",async()=>{
    platform("Win32");
    const search=await open();
    fireEvent.click(screen.getByRole("button",{name:"Settings"}));
    expect(screen.queryByText("Track a")).toBeNull();
    const event=press({key:"f",ctrlKey:true});
    expect(event.defaultPrevented).toBe(true);
    expect(search).toHaveFocus();
    expect(await screen.findByText("Track a")).toBeInTheDocument();
  });
  it("leaves the sidebar toggle on Cmd or Ctrl on every system",async()=>{
    for(const name of ["MacIntel","Win32"]){
      platform(name);
      const {container,unmount}=render(<App bridge={fakeBridge([track("a")])}/>);
      await screen.findByText("Track a");
      const app=container.querySelector(".app")!;
      const before=app.getAttribute("data-collapsed");
      let event!:KeyboardEvent;
      act(()=>{event=press({key:"b",ctrlKey:true});});
      expect(event.defaultPrevented,name).toBe(true);
      expect(app.getAttribute("data-collapsed"),name).not.toBe(before);
      act(()=>{event=press({key:"b",metaKey:true});});
      expect(event.defaultPrevented,name).toBe(true);
      expect(app.getAttribute("data-collapsed"),name).toBe(before);
      unmount();
      localStorage.clear();
    }
  });
});
