import {fireEvent,render,screen,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import InspectorFrame from "../components/InspectorFrame";

describe("selection inspector frame",()=>{
  it("focuses its heading and waits for durable navigation before closing",async()=>{
    let finish!:()=>void;
    const durable=new Promise<void>(resolve=>{finish=resolve;});
    const closed=vi.fn();
    render(<InspectorFrame title="Edit metadata" onClose={async()=>{await durable;closed();}}><input aria-label="Title"/></InspectorFrame>);
    expect(screen.getByRole("heading",{name:"Edit metadata"})).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("textbox",{name:"Title"}),{key:"Escape"});
    expect(closed).not.toHaveBeenCalled();
    expect(screen.getByRole("button",{name:"Back to tracks"})).toBeDisabled();
    finish();
    await waitFor(()=>expect(closed).toHaveBeenCalledTimes(1));
  });
  it("keeps unsaved content visible when native draft persistence rejects navigation",async()=>{
    const close=vi.fn(async()=>{throw Error("Draft storage is unavailable");});
    render(<InspectorFrame title="Edit metadata" onClose={close}><input aria-label="Title" defaultValue="Retain this edit"/></InspectorFrame>);
    fireEvent.click(screen.getByRole("button",{name:"Back to tracks"}));
    expect(await screen.findByRole("alert")).toHaveTextContent("Draft storage is unavailable");
    expect(screen.getByRole("textbox",{name:"Title"})).toHaveValue("Retain this edit");
    expect(screen.getByRole("button",{name:"Back to tracks"})).toBeEnabled();
  });
});
