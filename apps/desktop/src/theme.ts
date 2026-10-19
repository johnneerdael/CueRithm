import {useCallback,useEffect,useState,useSyncExternalStore} from "react";
export type ThemeMode="system"|"light"|"dark";
export type Theme="light"|"dark";
const key="cuerithm-theme";
export function readMode():ThemeMode{try{const v=localStorage.getItem(key);return v==="light"||v==="dark"?v:"system";}catch{return "system";}}
const query=()=>typeof window!=="undefined"&&typeof window.matchMedia==="function"?window.matchMedia("(prefers-color-scheme: dark)"):null;
export const resolveTheme=(mode:ThemeMode,prefersDark:boolean):Theme=>mode==="system"?(prefersDark?"dark":"light"):mode;
const next:Record<ThemeMode,ThemeMode>={system:"light",light:"dark",dark:"system"};
// The theme currently on the document, for components that draw their own colours (waveforms).
// Unlike useTheme it only reads, so any number of components can use it.
const applied=():Theme=>document.documentElement.dataset.theme==="light"?"light":"dark";
export function useAppliedTheme():Theme{
  return useSyncExternalStore(listener=>{const watch=new MutationObserver(listener);watch.observe(document.documentElement,{attributes:true,attributeFilter:["data-theme"]});return()=>watch.disconnect();},applied,()=>"dark");
}
export function useTheme(){
  const [mode,setMode]=useState<ThemeMode>(readMode);
  const [prefersDark,setPrefersDark]=useState(()=>query()?.matches??true);
  useEffect(()=>{
    const media=query();if(!media)return;
    const change=(event:MediaQueryListEvent)=>setPrefersDark(event.matches);
    media.addEventListener("change",change);return()=>media.removeEventListener("change",change);
  },[]);
  const theme=resolveTheme(mode,prefersDark);
  useEffect(()=>{document.documentElement.dataset.theme=theme;},[theme]);
  useEffect(()=>{try{if(mode==="system")localStorage.removeItem(key);else localStorage.setItem(key,mode);}catch{}},[mode]);
  const cycle=useCallback(()=>setMode(current=>next[current]),[]);
  return {mode,theme,cycle};
}
