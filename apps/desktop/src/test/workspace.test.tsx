import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";

describe("library workspace",()=>{
  it("retries cover extraction after a transient failure and remount",async()=>{
    const bridge=fakeBridge([track("a")]);
    bridge.getArtwork=vi.fn().mockRejectedValueOnce(new Error("Drive offline")).mockResolvedValue("data:image/png;base64,iVBORw0KGgo=");
    const first=render(<App bridge={bridge}/>);
    await waitFor(()=>expect(bridge.getArtwork).toHaveBeenCalledTimes(1));
    first.unmount();render(<App bridge={bridge}/>);
    await screen.findByRole("img",{name:"Cover art"});
    expect(bridge.getArtwork).toHaveBeenCalledTimes(2);
  });
  it("shows embedded cover art in the library and player",async()=>{
    const bridge=fakeBridge([track("a")]);
    bridge.getArtwork=vi.fn(async()=>"data:image/png;base64,iVBORw0KGgo=");
    render(<App bridge={bridge}/>);
    await screen.findByRole("img",{name:"Cover art"});
    fireEvent.doubleClick(screen.getByText("Track a"));
    await waitFor(()=>expect(screen.getAllByRole("img",{name:"Cover art"})).toHaveLength(2));
  });
  it("selects matching tracks beyond the loaded page with desktop shortcuts",async()=>{
    const bridge=fakeBridge([track("a")]);
    bridge.selectTrackIds=vi.fn(async()=>Array.from({length:450},(_,i)=>String(i)));
    render(<App bridge={bridge}/>);
    await screen.findByText("Track a");
    const table=screen.getByRole("table");
    for(const modifier of ["metaKey","ctrlKey"]){
      const event=new KeyboardEvent("keydown",{key:"a",[modifier]:true,bubbles:true,cancelable:true});
      table.dispatchEvent(event);
      expect(event.defaultPrevented).toBe(true);
      await screen.findByText("450 selected");
    }
    const before=vi.mocked(bridge.selectTrackIds).mock.calls.length;
    fireEvent.keyDown(screen.getByRole("searchbox"),{key:"a",metaKey:true});
    expect(bridge.selectTrackIds).toHaveBeenCalledTimes(before);
  });
  it("keeps selection separate from playback and updates committed Incoming tracks",async()=>{
    const bridge=fakeBridge([track("a"),track("b")]);
    render(<App bridge={bridge}/>);
    await screen.findByText("Track a");
    fireEvent.click(screen.getByText("Track b"));
    expect(bridge.playerLoad).not.toHaveBeenCalled();
    fireEvent.doubleClick(screen.getByText("Track a"));
    await waitFor(()=>expect(bridge.playerLoad).toHaveBeenCalledWith("a"));
    fireEvent.click(screen.getByRole("button",{name:/Incoming/}));
    bridge.rows=[{...track("incoming"),state:"incoming"}];
    bridge.changed({state:"running",stage:"analysis",track_id:"incoming",job_id:"job",sequence:1});
    await screen.findByText("Track incoming");
    bridge.rows=[{...track("incoming"),state:"ready"}];
    bridge.changed({state:"completed",stage:"completed",track_id:"incoming",job_id:"job",sequence:2});
    await waitFor(()=>expect(screen.queryByText("Track incoming")).not.toBeInTheDocument());
  });
  it("has one-time Settings instead of an import tagging wizard",async()=>{
    const bridge=fakeBridge([]);
    render(<App bridge={bridge}/>);
    await screen.findByText("Your library starts here");
    fireEvent.click(screen.getByRole("button",{name:"Settings"}));
    await screen.findByLabelText("Minimum breakdown length");
    expect(screen.queryByLabelText(/Energy model|OpenKeyScan model/)).not.toBeInTheDocument();
    expect(screen.getByText(/All analysis engines are included/)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Minimum breakdown length"),{target:{value:"128"}});
    fireEvent.click(screen.getByRole("button",{name:/Save settings/}));
    await waitFor(()=>expect(bridge.saveSettings).toHaveBeenCalled());
    expect(screen.queryByText("Select platforms")).not.toBeInTheDocument();
  });
  it("shows errors as actionable Incoming work",async()=>{
    const bridge=fakeBridge([{...track("bad"),state:"incoming"}]);
    bridge.jobs=[{id:"job",track_id:"bad",title:"Track bad",state:"attention",stage:"setup",error:"Choose an energy model in Settings",attempts:1}];
    render(<App bridge={bridge}/>);
    fireEvent.click(screen.getByRole("button",{name:/Incoming/}));
    await screen.findByText("Choose an energy model in Settings");
    expect(screen.getByRole("button",{name:/Retry/})).toBeEnabled();
  });
  it("virtualizes a large library and retains search focus during background updates",async()=>{
    const bridge=fakeBridge(Array.from({length:10000},(_,i)=>track(String(i))));
    render(<App bridge={bridge}/>);
    await screen.findByText("Track 0");
    expect(screen.getAllByRole("row").length).toBeLessThan(100);
    const search=screen.getByRole("searchbox");
    search.focus();
    bridge.changed({state:"running",stage:"analysis",track_id:"x",job_id:"x",sequence:1});
    await waitFor(()=>expect(search).toHaveFocus());
  });
  it("retains selected track identity while changing sort",async()=>{
    const bridge=fakeBridge([track("a"),track("b")]);
    render(<App bridge={bridge}/>);
    await screen.findByText("Track b");
    fireEvent.click(screen.getByText("Track b"));
    fireEvent.click(screen.getByRole("button",{name:"ARTIST"}));
    await waitFor(()=>expect(screen.getByText("Track b").closest('[role="row"]')).toHaveAttribute("aria-selected","true"));
  });
  it("saves visible column order without changing track identity",async()=>{
    const bridge=fakeBridge([track("a")]);
    render(<App bridge={bridge}/>);
    await screen.findByText("Track a");
    fireEvent.click(screen.getByRole("button",{name:"Columns"}));
    fireEvent.click(screen.getByRole("checkbox",{name:"Genre"}));
    expect(screen.queryByRole("columnheader",{name:"GENRE"})).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Move BPM left"}));
    expect(localStorage.getItem("track-columns")).toContain("bpm");
    expect(screen.getByText("Track a")).toBeInTheDocument();
  });
});
