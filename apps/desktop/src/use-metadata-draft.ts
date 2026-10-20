import {useEffect,useRef,useState} from "react";
import {errorMessage} from "./api";
import type {MetadataDraft,MetadataEditorBridge,MetadataEditorPatch} from "./editor-types";

export function useMetadataDraft(bridge:MetadataEditorBridge,initial:MetadataDraft){
  const [draft,setDraft]=useState(initial),[patch,setPatch]=useState(initial.patch),[pending,setPending]=useState(false),[error,setError]=useState<string|null>(null),[discarded,setDiscarded]=useState(false);
  const saved=useRef(initial),current=useRef(initial.patch),sequence=useRef(0),durable=useRef(0),alive=useRef(true),abandoning=useRef(false);
  const queue=useRef<Promise<MetadataDraft>>(Promise.resolve(initial));
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  const edit=(value:MetadataEditorPatch|((patch:MetadataEditorPatch)=>MetadataEditorPatch))=>{
    if(abandoning.current)return;
    setDiscarded(false);
    const next=typeof value==="function"?value(current.current):value;
    current.current=next;const generation=++sequence.current;
    setPatch(next);setPending(true);setError(null);
    const request=queue.current.catch(()=>saved.current).then(async()=>{
      const result=await bridge.updateMetadataDraft(initial.id,saved.current.revision,next);
      saved.current=result;durable.current=generation;
      if(alive.current&&generation===sequence.current){setDraft(result);setPending(false);setError(null);}
      return result;
    }).catch(failure=>{
      if(alive.current&&generation===sequence.current){setPending(false);setError(errorMessage(failure));}
      throw failure;
    });
    queue.current=request;
    void request.catch(()=>{});
  };
  const flush=async()=>{
    for(;;){
      const pending=queue.current;
      await pending;
      if(pending!==queue.current)continue;
      if(durable.current<sequence.current)throw Error("The latest edit has not been saved to its native draft.");
      return saved.current;
    }
  };
  const discardUnsaved=async()=>{
    abandoning.current=true;
    try{
      for(;;){const last=queue.current;await last.catch(()=>{});if(last===queue.current)break;}
      const acknowledged=saved.current;current.current=acknowledged.patch;durable.current=++sequence.current;
      queue.current=Promise.resolve(acknowledged);
      if(alive.current){setDraft(acknowledged);setPatch(acknowledged.patch);setPending(false);setError(null);setDiscarded(true);}
    }finally{abandoning.current=false;}
  };
  return {draft,patch,pending,error,discarded,discardUnsaved,dirty:durable.current<sequence.current,sequence:sequence.current,stamp:()=>sequence.current+":"+saved.current.revision,edit,flush,retry:()=>edit(current.current)};
}
