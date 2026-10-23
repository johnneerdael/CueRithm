import {getVersion} from "@tauri-apps/api/app";
import {useEffect,useState} from "react";
// The packaged app reports the release version stamped into its Tauri config. Outside Tauri (tests, a
// plain browser) there is no host to ask, so callers get null and show no number instead of a stale one.
export function useAppVersion():string|null{
  const [version,setVersion]=useState<string|null>(null);
  useEffect(()=>{
    let current=true;
    void getVersion().then(value=>{if(current&&value)setVersion(value);},()=>{});
    return()=>{current=false;};
  },[]);
  return version;
}
