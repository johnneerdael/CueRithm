import {useId,useLayoutEffect,useRef,useState,type ReactNode,type KeyboardEventHandler} from "react";
import {ArrowLeft,LoaderCircle} from "lucide-react";
import {errorMessage} from "../api";

export default function InspectorFrame({title,description,children,actions,navigation,onClose,onKeyDown}:{title:string;description?:string;children:ReactNode;actions?:ReactNode;navigation?:ReactNode;onClose:()=>Promise<void>|void;onKeyDown?:KeyboardEventHandler<HTMLElement>}){
  const id=useId(),heading=useRef<HTMLHeadingElement>(null),closing=useRef(false),alive=useRef(true);
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null);
  useLayoutEffect(()=>{alive.current=true;heading.current?.focus();return()=>{alive.current=false;};},[title]);
  const close=async()=>{
    if(closing.current)return;
    closing.current=true;setBusy(true);setError(null);
    try{await onClose();}catch(e){if(alive.current)setError(errorMessage(e));}
    finally{closing.current=false;if(alive.current)setBusy(false);}
  };
  return <section className="metadata-inspector" role="region" aria-labelledby={id} onKeyDown={event=>{onKeyDown?.(event);if(event.key==="Escape"&&!event.defaultPrevented&&!(event.target instanceof Element&&event.target.closest("[role=dialog],[role=menu]"))){event.preventDefault();event.stopPropagation();void close();}}}>
    <header className="inspector-header"><button className="inspector-back" disabled={busy} onClick={()=>void close()}>{busy?<LoaderCircle size={14} className="spin"/>:<ArrowLeft size={14}/>}Back to tracks</button><h2 id={id} ref={heading} tabIndex={-1}>{title}</h2>{description?<p>{description}</p>:null}{navigation}</header>
    {error?<div role="alert" className="inspector-error">{error}</div>:null}
    <div className="inspector-content">{children}</div>
    {actions?<footer className="inspector-actions">{actions}</footer>:null}
  </section>;
}
