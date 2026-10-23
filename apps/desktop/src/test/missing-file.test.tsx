import {render,screen,waitFor} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import App from "../App";
import RowPreview,{FILE_NOT_FOUND,FileNotFoundError} from "../components/RowPreview";
import {fakeBridge,track} from "./bridge";

describe("a track whose file is not there",()=>{
  it("says so in the Tracks table instead of showing a bare dash",async()=>{
    const bridge=fakeBridge([track("here"),track("gone")]);
    bridge.getTrack=vi.fn(async id=>({track:track(id),analysis:null,source_sha256:"hash",pcm_sha256:"pcm",file_available:id!=="gone"}));
    render(<App bridge={bridge}/>);
    const note=await screen.findByRole("img",{name:"File not found - move it back or run Maintenance"},{timeout:3000});
    expect(note).toHaveAttribute("title","File not found - move it back or run Maintenance");
    expect(note).toHaveTextContent("—");
    // No waveform is requested for a file that is known to be gone.
    await waitFor(()=>expect(bridge.getWaveform).toHaveBeenCalledWith("here",240));
    expect(bridge.getWaveform).not.toHaveBeenCalledWith("gone",240);
    expect(screen.getAllByRole("img",{name:FILE_NOT_FOUND})).toHaveLength(1);
  });
  it("does not call any other failure a missing file",async()=>{
    const {container}=render(<RowPreview id="x" identity="a" load={async()=>{throw new Error("decoder exploded");}}/>);
    await waitFor(()=>expect(container.querySelector(".dim")).not.toBeNull(),{timeout:2000});
    expect(container.querySelector(".dim")).toHaveTextContent("—");
    expect(screen.queryByRole("img",{name:FILE_NOT_FOUND})).toBeNull();
    // It still explains itself, with the wording for a request that failed, not for a gone file.
    expect(container.querySelector("[title]")).toHaveAttribute("title","No preview: it could not be loaded.");
  });
  it("names the problem the same way wherever it is raised",()=>{
    expect(new FileNotFoundError().message).toBe(FILE_NOT_FOUND);
  });
});
