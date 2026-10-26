import "@testing-library/jest-dom/vitest";
import {afterEach} from "vitest";
import {cleanup,configure} from "@testing-library/react";
afterEach(cleanup);
// A findBy* or waitFor waits for a condition and returns the moment it holds, so a long limit costs nothing when it passes.
// The 1 s default is shorter than a slow or busy CI runner (Windows most often) needs to render a large view, and the
// tests that failed on it were different ones each time, which only a limit set here can fix.
configure({asyncUtilTimeout:5000});
const values=new Map<string,string>();
Object.defineProperty(globalThis,"localStorage",{configurable:true,value:{
  getItem:(key:string)=>values.get(key)??null,setItem:(key:string,value:string)=>{values.set(key,String(value));},
  removeItem:(key:string)=>{values.delete(key);},clear:()=>values.clear(),key:(index:number)=>[...values.keys()][index]??null,get length(){return values.size;},
}});
globalThis.ResizeObserver=class {observe(){} unobserve(){} disconnect(){}} as unknown as typeof ResizeObserver;
