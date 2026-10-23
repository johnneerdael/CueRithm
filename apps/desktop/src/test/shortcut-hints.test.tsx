import {fireEvent,render,screen} from "@testing-library/react";
import {afterEach,describe,expect,it,vi} from "vitest";
import App from "../App";
import {isMacPlatform,shortcut} from "../platform";
import {fakeBridge,track} from "./bridge";

const platform=(name:string)=>vi.spyOn(window.navigator,"platform","get").mockReturnValue(name);
afterEach(()=>{vi.restoreAllMocks();});

describe("shortcut hints",()=>{
  it("pick the modifier from the platform and default to Ctrl when it is unknown",()=>{
    platform("MacIntel");expect([isMacPlatform(),shortcut("F"),shortcut("F",true)]).toEqual([true,"⌘F","⌘ F"]);
    for(const name of ["Win32","Linux x86_64","",]){
      platform(name);expect([isMacPlatform(),shortcut("F"),shortcut("F",true)]).toEqual([false,"Ctrl+F","Ctrl+F"]);
    }
  });
  it("prefer the client-hints platform when the webview offers it",()=>{
    platform("Win32");
    Object.defineProperty(window.navigator,"userAgentData",{configurable:true,value:{platform:"macOS"}});
    try{expect(isMacPlatform()).toBe(true);}finally{Reflect.deleteProperty(window.navigator,"userAgentData");}
  });
  it("show the Command key on macOS",async()=>{
    platform("MacIntel");
    const {container}=render(<App bridge={fakeBridge([track("a")])}/>);
    await screen.findByText("Track a");
    expect(container.querySelector(".global-search kbd")).toHaveTextContent("⌘ F");
    expect(screen.getByRole("button",{name:"Collapse sidebar"})).toHaveAttribute("title","Collapse to icons (⌘B)");
  });
  it.each(["Win32","Linux x86_64"])("show Ctrl on %s",async name=>{
    platform(name);
    const {container}=render(<App bridge={fakeBridge([track("a")])}/>);
    await screen.findByText("Track a");
    expect(container.querySelector(".global-search kbd")).toHaveTextContent("Ctrl+F");
    expect(container.querySelector(".global-search kbd")).not.toHaveTextContent("⌘");
    expect(screen.getByRole("button",{name:"Collapse sidebar"})).toHaveAttribute("title","Collapse to icons (Ctrl+B)");
  });
  it("focus the search box with Ctrl+F or Cmd+F, whatever the letter case",async()=>{
    render(<App bridge={fakeBridge([track("a")])}/>);
    await screen.findByText("Track a");
    const search=screen.getByRole("searchbox",{name:"Search library"});
    for(const init of [{key:"f",ctrlKey:true},{key:"f",metaKey:true},{key:"F",ctrlKey:true}]){
      search.blur();expect(search).not.toHaveFocus();
      const event=new KeyboardEvent("keydown",{...init,bubbles:true,cancelable:true});
      window.dispatchEvent(event);
      expect(event.defaultPrevented,JSON.stringify(init)).toBe(true);
      expect(search,JSON.stringify(init)).toHaveFocus();
    }
    search.blur();
    fireEvent.keyDown(window,{key:"f"});
    expect(search).not.toHaveFocus();
  });
});
