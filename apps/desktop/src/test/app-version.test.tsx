import {render,screen,waitFor} from "@testing-library/react";
import {beforeEach,describe,expect,it,vi} from "vitest";
import {getVersion} from "@tauri-apps/api/app";
import App from "../App";
import {fakeBridge,track} from "./bridge";

vi.mock("@tauri-apps/api/app",()=>({getVersion:vi.fn()}));
beforeEach(()=>{vi.mocked(getVersion).mockReset();});

describe("sidebar version label",()=>{
  it("shows the version the app reports instead of a fixed number",async()=>{
    vi.mocked(getVersion).mockResolvedValue("1.2.3");
    const {container}=render(<App bridge={fakeBridge([track("a")])}/>);
    await screen.findByText("Track a");
    await waitFor(()=>expect(container.querySelector(".build-label")).toHaveTextContent("CUERITHM 1.2.3"));
    expect(container.querySelector(".build-label")).not.toHaveTextContent("0.1");
  });
  it("shows no number outside Tauri, where the version cannot be read",async()=>{
    vi.mocked(getVersion).mockRejectedValue(new TypeError("Cannot read properties of undefined (reading 'invoke')"));
    const {container}=render(<App bridge={fakeBridge([track("a")])}/>);
    await screen.findByText("Track a");
    await waitFor(()=>expect(getVersion).toHaveBeenCalled());
    expect(container.querySelector(".build-label")).toHaveTextContent(/^CUERITHM$/);
    expect(container.querySelector(".build-label span")).toBeNull();
  });
});
