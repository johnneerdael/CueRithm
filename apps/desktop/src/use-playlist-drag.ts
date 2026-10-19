import {useEffect,useLayoutEffect,useRef,useState} from "react";
import type {PlaylistDragPayload,PlaylistDropTarget} from "./playlist-drag";
interface Session {payload:PlaylistDragPayload;source:HTMLElement;pointer:number;startX:number;startY:number;x:number;y:number;started:boolean;hover:HTMLElement|null;target:PlaylistDropTarget|null}
function hit(x:number,y:number){
 const element=document.elementFromPoint(x,y);if(!element)return null;
 const track=element.closest<HTMLElement>("[data-track-drop-id]");if(track)return {element:track,target:{kind:"track",id:track.dataset.trackDropId!} as PlaylistDropTarget};
 const playlist=element.closest<HTMLElement>("[data-playlist-drop-id]");if(playlist)return {element:playlist,target:{kind:"playlist",id:playlist.dataset.playlistDropId!} as PlaylistDropTarget};
 const root=element.closest<HTMLElement>("[data-playlist-drop-root]");return root?{element:root,target:{kind:"root"} as PlaylistDropTarget}:null;
}
export function usePlaylistDrag(options:{canDrop:(payload:PlaylistDragPayload,target:PlaylistDropTarget)=>boolean;onStart:(payload:PlaylistDragPayload)=>void;onDrop:(payload:PlaylistDragPayload,target:PlaylistDropTarget)=>void}){
 const handlers=useRef(options);handlers.current=options;
 const session=useRef<Session|null>(null),frame=useRef<number|null>(null),suppressed=useRef<HTMLElement|null>(null),clickTimer=useRef<ReturnType<typeof setTimeout>|null>(null);
 const active=useRef(false),ghost=useRef<HTMLDivElement>(null),[drag,setDrag]=useState<PlaylistDragPayload|null>(null);
 const position=()=>{const s=session.current;if(s&&ghost.current){ghost.current.style.left=`${s.x+12}px`;ghost.current.style.top=`${s.y+12}px`;}};
 const targetAt=(s:Session)=>{const result=hit(s.x,s.y),accepted=result&&handlers.current.canDrop(s.payload,result.target);const element=accepted?result.element:null;if(s.hover!==element){s.hover?.removeAttribute("data-internal-drop-target");element?.setAttribute("data-internal-drop-target","true");}s.hover=element;s.target=accepted?result.target:null;};
 const stop=(render=true)=>{const s=session.current;session.current=null;active.current=false;if(frame.current!==null){cancelAnimationFrame(frame.current);frame.current=null;}s?.hover?.removeAttribute("data-internal-drop-target");if(s?.source.hasPointerCapture?.(s.pointer))s.source.releasePointerCapture(s.pointer);if(render&&s?.started)setDrag(null);};
 useLayoutEffect(position,[drag]);
 useEffect(()=>{
  const scroll=()=>{frame.current=null;const s=session.current;if(!s?.started)return;const element=document.elementFromPoint(s.x,s.y)?.closest<HTMLElement>(".table-body,.sidebar-playlists,.sidebar");if(!element||element.scrollHeight<=element.clientHeight)return;const rect=element.getBoundingClientRect();if(!rect.height)return;const amount=s.y<rect.top+24?-12:s.y>rect.bottom-24?12:0;if(!amount)return;const previous=element.scrollTop;element.scrollTop+=amount;if(element.scrollTop!==previous){targetAt(s);frame.current=requestAnimationFrame(scroll);}};
  const move=(event:PointerEvent)=>{const s=session.current;if(!s||(event.pointerId??1)!==s.pointer)return;s.x=event.clientX;s.y=event.clientY;if(!s.started){if(Math.hypot(s.x-s.startX,s.y-s.startY)<6)return;s.started=true;active.current=true;s.source.setPointerCapture?.(s.pointer);handlers.current.onStart(s.payload);setDrag(s.payload);}event.preventDefault();targetAt(s);position();if(frame.current===null)frame.current=requestAnimationFrame(scroll);};
  const up=(event:PointerEvent)=>{const s=session.current;if(!s||(event.pointerId??1)!==s.pointer)return;let target:PlaylistDropTarget|null=null;if(s.started){s.x=event.clientX;s.y=event.clientY;targetAt(s);target=s.target;event.preventDefault();suppressed.current=s.source;if(clickTimer.current)clearTimeout(clickTimer.current);clickTimer.current=setTimeout(()=>{suppressed.current=null;},0);}stop();if(s.started&&target)handlers.current.onDrop(s.payload,target);};
  const cancel=()=>stop();const key=(event:KeyboardEvent)=>{if(event.key==="Escape"&&session.current){event.preventDefault();stop();}};
  window.addEventListener("pointermove",move,{passive:false});window.addEventListener("pointerup",up);window.addEventListener("pointercancel",cancel);window.addEventListener("blur",cancel);window.addEventListener("keydown",key);
  return()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",up);window.removeEventListener("pointercancel",cancel);window.removeEventListener("blur",cancel);window.removeEventListener("keydown",key);stop(false);if(clickTimer.current)clearTimeout(clickTimer.current);};
 },[]);
 const begin=(event:React.PointerEvent,payload:PlaylistDragPayload)=>{if(event.button!==0||event.isPrimary===false)return;stop();session.current={payload,source:event.currentTarget as HTMLElement,pointer:event.pointerId??1,startX:event.clientX,startY:event.clientY,x:event.clientX,y:event.clientY,started:false,hover:null,target:null};};
 const suppressClick=(event:React.MouseEvent)=>{if(suppressed.current?.contains(event.target as Node)){event.preventDefault();event.stopPropagation();}};
 return {drag,ghost,active,begin,suppressClick};
}
