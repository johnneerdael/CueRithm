import {fireEvent,render,screen} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import CategoryChoices from "../components/CategoryChoices";
describe("Curate category choices",()=>{
 it("applies a mixed choice to all without replacing unrelated choices",()=>{
  const toggle=vi.fn();render(<CategoryChoices name="Mood" mode="multi" choices={[{id:"warm",name:"Warm"},{id:"dark",name:"Dark"}]} selected={{warm:"some",dark:"all"}} onToggle={toggle}/>);
  const warm=screen.getByRole("checkbox",{name:"Warm"});expect(warm).toHaveAttribute("aria-checked","mixed");fireEvent.click(warm);
  expect(toggle).toHaveBeenCalledWith("warm",true);expect(toggle).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("checkbox",{name:"Dark"})).toHaveAttribute("aria-checked","true");
 });
 it("supports explicit removal and disables mutation while the draft cannot be edited",()=>{
  const toggle=vi.fn(),props={name:"Set moment",mode:"single" as const,choices:[{id:"open",name:"Opening"}],selected:{open:"all" as const},onToggle:toggle};
  const view=render(<CategoryChoices {...props}/>);fireEvent.click(screen.getByRole("checkbox",{name:"Opening"}));expect(toggle).toHaveBeenCalledWith("open",false);
  view.rerender(<CategoryChoices {...props} disabled/>);fireEvent.click(screen.getByRole("checkbox",{name:"Opening"}));expect(toggle).toHaveBeenCalledTimes(1);
 });
});
