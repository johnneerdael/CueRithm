import {useEffect,useState,type ReactNode} from "react";
import type {Bridge} from "../types";
type Loader=Bridge["getArtwork"];
const caches=new WeakMap<Loader,Map<string,Promise<string|null>>>();
export default function Artwork({id,identity,load,fallback}:{id:string;identity:string;load?:Loader;fallback:ReactNode}){
  const [url,setUrl]=useState<string|null>(null);
  useEffect(()=>{
    let current=true;setUrl(null);
    if(!load||!identity)return;
    let cache=caches.get(load);if(!cache){cache=new Map();caches.set(load,cache);}
    let pending=cache.get(identity);
    if(!pending){pending=load(id,identity).catch(error=>{cache.delete(identity);throw error;});cache.set(identity,pending);if(cache.size>64)cache.delete(cache.keys().next().value!);}
    void pending.then(value=>{if(current)setUrl(value);}).catch(()=>{});
    return()=>{current=false;};
  },[id,identity,load]);
  return url?<img src={url} alt="Cover art" draggable={false} onError={()=>setUrl(null)}/>:fallback;
}
