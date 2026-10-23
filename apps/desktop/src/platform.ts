// Words for the shortcut hints. The key handlers accept Cmd or Ctrl on every system; this only picks
// what the hint says. WKWebView reports "MacIntel" on Apple silicon; anything unknown gets Ctrl.
export function isMacPlatform():boolean{
  try{
    const nav=globalThis.navigator as (Navigator&{userAgentData?:{platform?:string}})|undefined;
    return /mac|iphone|ipad/i.test(nav?.userAgentData?.platform||nav?.platform||"");
  }catch{return false;}
}
// The shortcut modifier alone: Cmd on a Mac, Ctrl elsewhere. A bare Ctrl+F on a Mac is the text field's own
// cursor-forward, and Cmd+Ctrl+F is the system full-screen key, so the other modifier must not be held.
export const hasShortcutModifier=(event:Pick<KeyboardEvent,"metaKey"|"ctrlKey">)=>isMacPlatform()?event.metaKey&&!event.ctrlKey:event.ctrlKey&&!event.metaKey;
export const shortcut=(key:string,spaced=false)=>isMacPlatform()?(spaced?"⌘ ":"⌘")+key:"Ctrl+"+key;
