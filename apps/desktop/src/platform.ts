// Words for the shortcut hints. The key handlers accept Cmd or Ctrl on every system; this only picks
// what the hint says. WKWebView reports "MacIntel" on Apple silicon; anything unknown gets Ctrl.
export function isMacPlatform():boolean{
  try{
    const nav=globalThis.navigator as (Navigator&{userAgentData?:{platform?:string}})|undefined;
    return /mac|iphone|ipad/i.test(nav?.userAgentData?.platform||nav?.platform||"");
  }catch{return false;}
}
export const shortcut=(key:string,spaced=false)=>isMacPlatform()?(spaced?"⌘ ":"⌘")+key:"Ctrl+"+key;
