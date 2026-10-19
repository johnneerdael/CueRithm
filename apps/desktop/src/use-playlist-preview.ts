import {useEffect,useRef,useState} from "react";
import type {PlaylistChange,SmartStamp} from "./playlist-types";
import {errorMessage} from "./api";
interface PinnedPreview {id:string;stamp:SmartStamp;expires:number|null}
export function usePlaylistPreview<Request,Preview extends PinnedPreview>({request,revision,prepare,apply,onApplied}:{request:Request|null;revision:number;prepare:(request:Request)=>Promise<Preview>;apply:(id:string,revision:number)=>Promise<PlaylistChange>;onApplied:(change:PlaylistChange)=>Promise<void>}){
 const signature=request===null?"":JSON.stringify(request),currentSignature=useRef(signature);currentSignature.current=signature;
 const [snapshot,setSnapshot]=useState<{signature:string;value:Preview}|null>(null),[busy,setBusy]=useState<"preview"|"apply"|null>(null),[error,setError]=useState<string|null>(null),[completed,setCompleted]=useState<PlaylistChange|null>(null);
 const alive=useRef(true),generation=useRef(0),running=useRef<"preview"|"apply"|null>(null);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;};},[]);
 useEffect(()=>{generation.current++;setSnapshot(null);setCompleted(null);setError(null);},[signature]);
 useEffect(()=>{if(snapshot&&snapshot.value.stamp.playlist_revision!==revision&&running.current!=="apply"){generation.current++;setSnapshot(null);setError("Playlists changed. Preview again before applying.");}},[snapshot,revision]);
 useEffect(()=>{const expires=snapshot?.value.expires;if(!expires)return;const timer=setTimeout(()=>{if(running.current!=="apply"){setSnapshot(null);setError("Preview expired. Preview again before applying.");}},Math.max(0,expires-Date.now()));return()=>clearTimeout(timer);},[snapshot]);
 const preview=snapshot&&snapshot.signature===signature&&snapshot.value.stamp.playlist_revision===revision&&(snapshot.value.expires===null||snapshot.value.expires>Date.now())?snapshot.value:null;
 const previewNow=async()=>{
  if(!request||running.current)return;running.current="preview";setBusy("preview");setError(null);setSnapshot(null);setCompleted(null);const token=++generation.current,expected=signature;
  try{const next=await prepare(request);if(alive.current&&token===generation.current&&expected===currentSignature.current)setSnapshot({signature:expected,value:next});}
  catch(e){if(alive.current&&token===generation.current)setError(errorMessage(e));}finally{running.current=null;if(alive.current)setBusy(null);}
 };
 const applyNow=async()=>{
  if(!preview||running.current)return;running.current="apply";setBusy("apply");setError(null);let committed=false;
  try{const change=await apply(preview.id,preview.stamp.playlist_revision);committed=true;if(alive.current){setSnapshot(null);setCompleted(change);}await onApplied(change);}
  catch(e){if(alive.current){setSnapshot(null);setError((committed?"Changes were applied, but refreshing failed: ":"")+errorMessage(e));}}
  finally{running.current=null;if(alive.current)setBusy(null);}
 };
 return {preview,busy,error,completed,previewNow,applyNow};
}
