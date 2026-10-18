import {it,expect} from "vitest";
import {camelotColor} from "../display";
it("uses both reference Camelot rings consistently and leaves invalid keys neutral",()=>{
  const colors=Array.from({length:12},(_,i)=>[camelotColor(`${i+1}A`),camelotColor(`${i+1}B`)]).flat();
  expect(new Set(colors).size).toBe(24);
  expect(camelotColor("6B")).toBe("#ff9da5");
  expect(camelotColor("8A")).toBe("#f7c4ec");
  expect(camelotColor(" 12b ")).toBe(camelotColor("12B"));
  for(const invalid of ["constructor","__proto__","13A","0B",null])expect(camelotColor(invalid)).toBeUndefined();
});
