import {render,screen,fireEvent} from "@testing-library/react";
import {describe,it,expect,vi} from "vitest";
import RecognitionSettings,{defaultRecognition} from "../components/RecognitionSettings";
describe("recording identification consent",()=>{
  it("starts off and requires fingerprint consent before enabling",()=>{
    const change=vi.fn();render(<RecognitionSettings configured onChange={change}/>);
    expect(screen.getByLabelText("Identify unresolved tracks")).toBeDisabled();
    fireEvent.click(screen.getByLabelText("Allow fingerprint lookup"));
    expect(change).toHaveBeenCalledWith({...defaultRecognition,fingerprint_consent:true});
  });
  it("removing consent disables identification in the same draft",()=>{
    const change=vi.fn();render(<RecognitionSettings configured config={{...defaultRecognition,enabled:true,fingerprint_consent:true}} onChange={change}/>);
    fireEvent.click(screen.getByLabelText("Allow fingerprint lookup"));
    expect(change).toHaveBeenCalledWith({...defaultRecognition});
  });
  it("missing application identity does not request a personal API key",()=>{
    render(<RecognitionSettings configured={false} onChange={vi.fn()}/>);
    expect(screen.getByRole("status")).toHaveTextContent("application identity is missing");
    expect(screen.getByLabelText("Allow fingerprint lookup")).toBeDisabled();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });
});
