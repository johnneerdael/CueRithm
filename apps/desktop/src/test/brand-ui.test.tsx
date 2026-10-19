import {render,screen,fireEvent,waitFor,within} from "@testing-library/react";
import {describe,it,expect,beforeEach,vi} from "vitest";
import App from "../App";
import {resolveTheme} from "../theme";
import {fakeBridge,track} from "./bridge";

beforeEach(()=>{localStorage.clear();delete document.documentElement.dataset.theme;});

describe("brand theme",()=>{
  it("resolves the system preference only in system mode",()=>{
    expect(resolveTheme("system",true)).toBe("dark");
    expect(resolveTheme("system",false)).toBe("light");
    expect(resolveTheme("light",true)).toBe("light");
    expect(resolveTheme("dark",false)).toBe("dark");
  });
  it("cycles system, light and dark, applies data-theme and remembers the choice",async()=>{
    render(<App bridge={fakeBridge([track("a")])}/>);
    await screen.findByText("Track a");
    fireEvent.click(screen.getByRole("button",{name:"Colour theme: system"}));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem("cuerithm-theme")).toBe("light");
    fireEvent.click(screen.getByRole("button",{name:"Colour theme: light"}));
    expect(document.documentElement.dataset.theme).toBe("dark");
    fireEvent.click(screen.getByRole("button",{name:"Colour theme: dark"}));
    expect(localStorage.getItem("cuerithm-theme")).toBeNull();
  });
});

describe("collapsible sidebar",()=>{
  it("collapses to a rail, keeps destinations reachable, toggles with ⌘B and persists",async()=>{
    const first=render(<App bridge={fakeBridge([track("a")])}/>);
    await screen.findByText("Track a");
    const app=first.container.querySelector(".app")!;
    expect(app).toHaveAttribute("data-collapsed","false");
    fireEvent.click(screen.getByRole("button",{name:"Collapse sidebar"}));
    expect(app).toHaveAttribute("data-collapsed","true");
    expect(first.container.querySelector(".sidebar")).toHaveClass("rail");
    expect(screen.getByRole("button",{name:/Incoming/})).toBeInTheDocument();
    fireEvent.keyDown(window,{key:"b",metaKey:true});
    expect(app).toHaveAttribute("data-collapsed","false");
    fireEvent.keyDown(window,{key:"b",metaKey:true});
    first.unmount();
    const second=render(<App bridge={fakeBridge([track("a")])}/>);
    await screen.findByText("Track a");
    expect(second.container.querySelector(".app")).toHaveAttribute("data-collapsed","true");
  });
  it("expands over the content while hovered and returns to the rail afterwards",async()=>{
    vi.useFakeTimers({shouldAdvanceTime:true});
    try{
      const {container}=render(<App bridge={fakeBridge([track("a")])}/>);
      await screen.findByText("Track a");
      fireEvent.click(screen.getByRole("button",{name:"Collapse sidebar"}));
      const sidebar=container.querySelector(".sidebar")!;
      fireEvent.mouseEnter(sidebar);
      await waitFor(()=>expect(sidebar).toHaveClass("peek"));
      fireEvent.mouseLeave(sidebar);
      expect(sidebar).toHaveClass("rail");
    }finally{vi.useRealTimers();}
  });
});

describe("track table layout",()=>{
  it("combines track and artist, keeps both sortable and shows each preparation outcome before the artwork",async()=>{
    const rows=[
      {...track("a"),title:"Afterglow",artist:"Northline",metadata:{"library.tag_status":"matched"}},
      {...track("b"),title:"Parallel",artist:"Vela",metadata:{"library.tag_status":"no_match"}},
      {...track("c"),title:"First Light",artist:"Solis",metadata:{"library.tag_status":"already_tagged"}},
      {...track("d"),title:"Untouched",artist:"Aster"},
    ];
    render(<App bridge={fakeBridge(rows)}/>);
    await screen.findByText("Afterglow");
    expect(screen.getByRole("button",{name:"TRACK"})).toBeInTheDocument();
    expect(screen.getByRole("button",{name:"ARTIST"})).toBeInTheDocument();
    expect(screen.queryByRole("columnheader",{name:"ARTIST"})).toBeNull();
    const cellOf=(title:string)=>screen.getByText(title).closest<HTMLElement>('[role="row"]')!;
    expect(within(cellOf("Afterglow")).getByText("Northline")).toBeInTheDocument();
    expect(within(cellOf("Afterglow")).getByRole("img",{name:"Tagged from catalog match"})).toBeInTheDocument();
    expect(within(cellOf("Parallel")).getByRole("img",{name:/No catalog match/})).toBeInTheDocument();
    expect(within(cellOf("First Light")).getByRole("img",{name:"Already tagged"})).toBeInTheDocument();
    expect(within(cellOf("Untouched")).queryByRole("img")).toBeNull();
    expect(within(cellOf("Afterglow")).getByText("8A").className).toContain("key-chip");
    expect(within(cellOf("Afterglow")).getByText("6/10")).toBeInTheDocument();
  });
  it("flags tracks that need attention instead of showing a tagging result",async()=>{
    const bridge=fakeBridge([{...track("a"),state:"incoming",metadata:{"library.tag_status":"matched"}}]);
    bridge.jobs=[{id:"j",track_id:"a",title:"Track a",state:"attention",stage:"matching",error:"No catalog match",attempts:1}];
    render(<App bridge={bridge}/>);
    fireEvent.click(await screen.findByRole("button",{name:/Incoming/}));
    const row=(await screen.findByText("Track a")).closest<HTMLElement>('[role="row"]')!;
    expect(within(row).getByRole("img",{name:"Needs attention"})).toHaveAttribute("title","No catalog match");
    expect(within(row).queryByRole("img",{name:"Tagged from catalog match"})).toBeNull();
  });
});
