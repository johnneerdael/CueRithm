import {useEffect,useRef,useState} from "react";
import {ImagePlus,LoaderCircle} from "lucide-react";
import type {MetadataArtworkChoice,MetadataEditorBridge,MetadataEditorSnapshot,ArtworkIdentity} from "../editor-types";
import {inspectorOperation} from "../inspector-operations";
import {errorMessage} from "../api";
const identity=(value:ArtworkIdentity|null)=>JSON.stringify(value);
const kindName=(kind:number|null)=>kind===3?"Front cover":kind===4?"Back cover":kind===8?"Artist":kind===6?"Media":"Picture";
export default function ArtworkEditor({bridge,snapshot,choices,onChange,disabled,onPendingInput}:{bridge:MetadataEditorBridge;snapshot:MetadataEditorSnapshot;choices:MetadataArtworkChoice[];onChange:(choices:MetadataArtworkChoice[])=>void;disabled:boolean;onPendingInput?:(pending:Promise<void>)=>void}){
 const pictures=(snapshot.tracks[0]?.native.artwork??[]).filter(picture=>snapshot.tracks.every(track=>track.native.artwork.some(other=>identity(other.identity)===identity(picture.identity))));
 const [selected,setSelected]=useState(pictures[0]?identity(pictures[0].identity):"new"),[image,setImage]=useState<string|null>(null),[error,setError]=useState<string|null>(null),[busy,setBusy]=useState(false),[kind,setKind]=useState<number|null>(null),[description,setDescription]=useState("");
 const current=useRef(choices);current.current=choices;
 const generation=useRef(0),read=useRef<string|null>(null),alive=useRef(true);
 const picture=pictures.find(value=>identity(value.identity)===selected),target=picture?.identity??null;
 const chosen=choices.find(choice=>identity(choice.target)===identity(target));
 const put=(choice:MetadataArtworkChoice|null,target:ArtworkIdentity|null)=>{const remaining=current.current.filter(value=>identity(value.target)!==identity(target));onChange(choice?[...remaining,choice]:remaining);};
 useEffect(()=>{
  alive.current=true;const request=++generation.current;setImage(null);setError(null);setBusy(false);
  if(chosen?.asset_id===null&&chosen)return;
  const load=async()=>{
   setBusy(true);
   try{
    let asset=chosen?.asset_id;
    if(!asset&&picture){const first=snapshot.tracks[0];asset=(await inspectorOperation(bridge,async id=>{if(request!==generation.current)throw Error("superseded");read.current=id;return bridge.extractMetadataArtwork(id,first.track_id,first.source_sha256,picture.identity);})).id;}
    if(!asset)return;
    const value=await bridge.metadataArtworkAsset(asset);
    if(alive.current&&request===generation.current){if(!value.startsWith("data:image/"))throw Error("Artwork preview has an unsupported image format.");setImage(value);}
   }catch(e){if(alive.current&&request===generation.current)setError(errorMessage(e));}
   finally{if(request===generation.current){read.current=null;if(alive.current)setBusy(false);}}
  };
  if(picture||chosen?.asset_id)void load();
  return()=>{generation.current++;if(read.current)void bridge.cancelMetadataOperation(read.current).catch(()=>{});};
 },[bridge,selected,chosen?.asset_id,snapshot]);
 useEffect(()=>()=>{alive.current=false;generation.current++;},[]);
 const chooseImage=async()=>{
  const capturedTarget=target;setError(null);setBusy(true);
  try{const asset=await bridge.chooseMetadataArtwork();if(asset&&alive.current)put({target:capturedTarget,asset_id:asset.id,kind:capturedTarget?.kind??kind,description:capturedTarget?.description??description},capturedTarget);}
  catch(e){if(alive.current)setError(errorMessage(e));}
  finally{if(alive.current)setBusy(false);}
 };
 const choose=()=>{const pending=chooseImage();onPendingInput?.(pending);return pending;};
 return <section className="metadata-section artwork-editor"><h3>Artwork</h3><div className="artwork-editor-grid"><div className="editor-cover">{busy?<LoaderCircle className="spin" size={24}/>:image?<img src={image} alt={picture?kindName(picture.identity.kind):"New cover artwork"}/>:<ImagePlus size={30}/>}</div><div><label>Selected picture<select aria-label="Selected picture" disabled={disabled||busy} value={selected} onChange={e=>setSelected(e.target.value)}>{pictures.map(value=><option key={identity(value.identity)} value={identity(value.identity)}>{kindName(value.identity.kind)}{value.identity.description?" · "+value.identity.description:""} · {value.identity.sha256.slice(0,8)}</option>)}<option value="new">Add a picture</option></select></label>{picture?<p className="setting-note">{picture.mime} · {Math.ceil(picture.bytes/1024)} KiB · picture {picture.identity.ordinal+1}</p>:<><label>Picture type<select aria-label="New picture type" disabled={disabled} value={kind??""} onChange={e=>{const value=e.target.value===""?null:Number(e.target.value);setKind(value);if(chosen)put({...chosen,kind:value},null);}}><option value="">Container default</option><option value={3}>Front cover</option><option value={4}>Back cover</option><option value={8}>Artist</option><option value={0}>Other</option></select></label><label>Description<input aria-label="New picture description" disabled={disabled} value={description} onChange={e=>{setDescription(e.target.value);if(chosen)put({...chosen,description:e.target.value},null);}}/></label></>}<div className="artwork-actions"><button className="secondary" disabled={disabled||busy} onClick={()=>void choose()}>{picture?"Replace selected picture":"Choose image"}</button>{picture?<button className="secondary" disabled={disabled||busy} onClick={()=>put({target:picture.identity,asset_id:null,kind:null,description:null},picture.identity)}>Remove selected picture</button>:null}{chosen?<button className="back-link" disabled={disabled||busy} onClick={()=>put(null,target)}>Keep original artwork</button>:null}</div>{chosen?<p role="status" className="setting-note">{chosen.asset_id?picture?"Selected picture will be replaced.":"A picture will be added.":"Selected picture will be removed."}</p>:null}</div></div>{snapshot.tracks.length>1?<p className="setting-note">Replace/remove applies only to identical pictures shared by all {snapshot.tracks.length} tracks. Other pictures stay intact. A new picture is added to every selected track.</p>:null}{error?<p role="alert" className="metadata-field-error">{error}</p>:null}</section>;
}
