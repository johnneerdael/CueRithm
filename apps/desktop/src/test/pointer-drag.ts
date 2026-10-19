import {fireEvent} from "@testing-library/react";
import {vi} from "vitest";
Object.defineProperty(window,"PointerEvent",{configurable:true,value:MouseEvent});
export function pointerDrag(from:Element,to:Element,cancel=false){
 const previous=Object.getOwnPropertyDescriptor(document,"elementFromPoint");
 Object.defineProperty(document,"elementFromPoint",{configurable:true,value:vi.fn(()=>to)});
 try{fireEvent.pointerDown(from,{button:0,buttons:1,pointerId:1,clientX:10,clientY:10});fireEvent.pointerMove(window,{button:0,buttons:1,pointerId:1,clientX:50,clientY:50});if(cancel)fireEvent.keyDown(window,{key:"Escape"});fireEvent.pointerUp(window,{button:0,buttons:0,pointerId:1,clientX:50,clientY:50});}
 finally{if(previous)Object.defineProperty(document,"elementFromPoint",previous);else Reflect.deleteProperty(document,"elementFromPoint");}
}
