import "@testing-library/jest-dom/vitest";
import {afterEach} from "vitest";
import {cleanup} from "@testing-library/react";
afterEach(cleanup);
const values=new Map<string,string>();
Object.defineProperty(globalThis,"localStorage",{configurable:true,value:{
  getItem:(key:string)=>values.get(key)??null,setItem:(key:string,value:string)=>{values.set(key,String(value));},
  removeItem:(key:string)=>{values.delete(key);},clear:()=>values.clear(),key:(index:number)=>[...values.keys()][index]??null,get length(){return values.size;},
}});
globalThis.ResizeObserver=class {observe(){} unobserve(){} disconnect(){}} as unknown as typeof ResizeObserver;
