export type NativePath=string|{unix:number[]}|{windows:number[]};

export function pathText(path:NativePath):string{
 if(typeof path==="string")return path;
 let text="";
 if("unix" in path){
  const bytes=Uint8Array.from(path.unix),decoder=new TextDecoder("utf-8",{fatal:true,ignoreBOM:true});
  for(let index=0;index<bytes.length;){
   let count=0;
   for(let length=1;length<=4&&index+length<=bytes.length;length++){
    try{text+=decoder.decode(bytes.subarray(index,index+length));count=length;break;}catch{/* Try the complete UTF-8 sequence before escaping a raw byte. */}
   }
   if(count)index+=count;else{text+=`\\x${bytes[index].toString(16).padStart(2,"0")}`;index++;}
  }
 }else{
  for(let index=0;index<path.windows.length;index++){
   const unit=path.windows[index],next=path.windows[index+1];
   if(unit>=0xd800&&unit<=0xdbff&&next>=0xdc00&&next<=0xdfff){text+=String.fromCharCode(unit,next);index++;}
   else if(unit>=0xd800&&unit<=0xdfff)text+=`\\u${unit.toString(16).padStart(4,"0")}`;
   else text+=String.fromCharCode(unit);
  }
 }
 return text;
}
export function pathKey(path:NativePath):string{return JSON.stringify(path);}
