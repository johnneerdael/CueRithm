import {describe,it,expect} from "vitest";
import {operators,allowedOperators,initialRuleValue,validRuleValue} from "../smart-rules";
describe("smart rule contracts",()=>{
  it("exposes all 58 source operators through valid typed field families",()=>{
    expect(Object.keys(operators)).toHaveLength(58);
    const covered=new Set<string>();
    for(const [field,kind] of [["title","text"],["AnyTextField","text_group"],["bpm","number"],["key","text"],["dateAdded","date"],["archived","boolean"],["color","color"],["tags","tags"],["categories","categories"],["playlist","playlist"],["cuepoints","number"],["tempomarkers","number"]]){
      for(const op of allowedOperators(field,kind))covered.add(op);
    }
    expect([...covered].sort()).toEqual(Object.keys(operators).sort());
    expect(allowedOperators("artist","text")).not.toContain("KeySimilar");
    expect(allowedOperators("energy","number")).not.toContain("BpmExactNumber");
    expect(allowedOperators("playlist","playlist")).not.toContain("Exists");
  });
  it("initializes tagged values with the correct arity and rejects unfinished numeric/reference input",()=>{
    for(const op of Object.keys(operators)){
      const value=initialRuleValue(op);
      expect(value).toHaveProperty("kind");
      if(value.kind!=="playlist")expect(validRuleValue(value)).toBe(true);
    }
    expect(validRuleValue({kind:"number",value:NaN})).toBe(false);
    expect(validRuleValue({kind:"number_range",value:{start:1,end:Infinity}})).toBe(false);
    expect(validRuleValue({kind:"playlist",value:{id:"",kind:"playlist",path:[]}})).toBe(false);
  });
});
