import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import {fakeBridge,track} from "./bridge";

describe("library workspace",()=>{
  it("offers metadata from the whole library and retains custom columns after remount",async()=>{
    const bridge=fakeBridge([{...track("a"),metadata:{"tag.label":"Example label","audio.sample_rate":44100}}]);
    bridge.getMetadataFields=vi.fn(async()=>["tag.label","tag.custom:mood","audio.sample_rate"]);
    const first=render(<App bridge={bridge}/>);
    await screen.findByText("Track a");
    fireEvent.click(screen.getByRole("button",{name:"Columns"}));
    fireEvent.click(await screen.findByRole("checkbox",{name:"Label"}));
    expect(screen.getByRole("checkbox",{name:"Custom:mood"})).toBeInTheDocument();
    expect(screen.getByRole("columnheader",{name:"LABEL"})).toBeInTheDocument();
    expect(screen.getByText("Example label")).toBeInTheDocument();
    first.unmount();render(<App bridge={bridge}/>);
    await screen.findByText("Example label");
    expect(screen.getByRole("columnheader",{name:"LABEL"})).toBeInTheDocument();
  });
  it("jumps one bar with arrow keys using current playback position, queues repeats and respects controls",async()=>{
    const bridge=fakeBridge([track("a")]);
    const load=bridge.playerLoad;
    bridge.playerLoad=vi.fn(async id=>({...await load(id),analysis:{complete:true,source_sample_rate:44100,analysis:{duration_seconds:240,key:null,energy:null,tempo:{bpm:120,beats_seconds:[0,.5,1],downbeats_seconds:null},lexicon_cues:null}}}));
    render(<App bridge={bridge}/>);
    await screen.findByText("Track a");
    fireEvent.doubleClick(screen.getByText("Track a"));
    await screen.findByRole("button",{name:"Pause"});
    await bridge.playerSeek(3.25);
    vi.mocked(bridge.playerSeek).mockClear();
    fireEvent.keyDown(window,{key:"ArrowRight"});
    fireEvent.keyDown(window,{key:"ArrowRight"});
    await waitFor(()=>expect(bridge.playerSeek).toHaveBeenCalledTimes(2));
    expect(bridge.playerSeek).toHaveBeenNthCalledWith(1,5.25);
    expect(bridge.playerSeek).toHaveBeenNthCalledWith(2,7.25);
    fireEvent.keyDown(screen.getByRole("slider",{name:"Track overview"}),{key:"ArrowLeft"});
    await waitFor(()=>expect(bridge.playerSeek).toHaveBeenLastCalledWith(5.25));
    expect(bridge.playerSeek).toHaveBeenCalledTimes(3);
    fireEvent.keyDown(screen.getByRole("searchbox"),{key:"ArrowRight"});
    fireEvent.keyDown(screen.getByRole("slider",{name:"Volume"}),{key:"ArrowRight"});
    fireEvent.keyDown(window,{key:"ArrowRight",metaKey:true});
    expect(bridge.playerSeek).toHaveBeenCalledTimes(3);
    await bridge.playerSeek(239);
    vi.mocked(bridge.playerSeek).mockClear();
    fireEvent.keyDown(window,{key:"ArrowRight"});
    await waitFor(()=>expect(bridge.playerSeek).toHaveBeenCalledWith(240));
    await bridge.playerSeek(.25);
    vi.mocked(bridge.playerSeek).mockClear();
    fireEvent.keyDown(window,{key:"ArrowLeft"});
    await waitFor(()=>expect(bridge.playerSeek).toHaveBeenCalledWith(0));
  });
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
    fireEvent.click(await screen.findByRole("button",{name:"Cue generation"}));
    await screen.findByLabelText("Minimum breakdown length");
    expect(screen.queryByLabelText(/Energy model|OpenKeyScan model/)).not.toBeInTheDocument();
    expect(screen.queryByRole("heading",{name:"Analysis"})).not.toBeInTheDocument();
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
