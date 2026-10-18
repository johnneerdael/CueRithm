import {useRef,useState,useLayoutEffect,useEffect,useMemo} from "react";
import {createPortal} from "react-dom";
import {Play,Music2,ChevronUp,ChevronDown,MoreHorizontal,RotateCcw,Archive,Columns3,ArrowUp,ArrowDown,X,LoaderCircle,Check} from "lucide-react";
import Artwork from "./Artwork";
import Glyph from "./Glyph";
import RowPreview,{type PreviewLoader} from "./RowPreview";
import type {Track,Sort,Job,View,Bridge} from "../types";
import {time} from "../display";
import {columnLabel as label,metadataText} from "../columns";
const height=56;
type Column=string;
const defaults:Column[]=["title","genre","bpm","key","energy","duration","preview"];
const layoutVersion=2;
const widths:Record<string,string>={title:"minmax(220px,2.4fr)",artist:"minmax(110px,1fr)",genre:"minmax(85px,.8fr)",bpm:"74px",key:"58px",energy:"96px",duration:"60px",artwork:"66px",preview:"minmax(150px,1fr)"};
const stages:Record<string,string>={queued:"Queued",waiting:"Waiting for file",decoding:"Decoding",analysis:"Analyzing",matching:"Matching catalog",tagging:"Preparing tags",writing_tags:"Writing tags",committing:"Applying cues",setup:"Setup"};
function preferences():{order:Column[];hidden:Column[]}{try{const v=JSON.parse(localStorage.getItem("track-columns")??"null");if(v&&v.version===layoutVersion&&Array.isArray(v.order)&&v.order.length&&v.order.every((c:unknown)=>typeof c==="string"))return {order:[...new Set<string>(v.order)],hidden:Array.isArray(v.hidden)?v.hidden.filter((c:unknown)=>typeof c==="string"):[]};}catch{}return {order:defaults,hidden:[]};}
export default function TrackTable({rows,selected,playingId,onSelect,onPlay,sort,descending,onSort,view,jobs,onRetry,onReprepare,onArchive,onMore,hasMore,loading,artworkLoader,metadataLoader,previewLoader,onTrackDragStart}:{onTrackDragStart?:(row:Track,event:React.PointerEvent)=>void;previewLoader?:PreviewLoader;metadataLoader?:()=>Promise<string[]>;artworkLoader?:Bridge["getArtwork"];rows:Track[];selected:Set<string>;playingId:string|null;onSelect:(row:Track,event:React.MouseEvent)=>void;onPlay:(row:Track)=>void;sort:Sort;descending:boolean;onSort:(sort:Sort)=>void;view:View;jobs:Job[];onRetry:(id:string)=>void;onReprepare:(id:string)=>void;onArchive:(id:string)=>void;onMore:()=>void;hasMore:boolean;loading:boolean}){
  const viewport=useRef<HTMLDivElement>(null),header=useRef<HTMLDivElement>(null);
  const [top,setTop]=useState(0),[prefs,setPrefs]=useState(preferences);
  const [menu,setMenu]=useState<{id:string;x:number;y:number}|null>(null);
  const [columnsPosition,setColumnsPosition]=useState({left:0,top:0});
  const [columns,setColumns]=useState(false),[columnSearch,setColumnSearch]=useState("");
  const [metadataFields,setMetadataFields]=useState<string[]>([]),[readingMetadata,setReadingMetadata]=useState(false),[metadataError,setMetadataError]=useState<string|null>(null);
  const metadataRequest=useRef(0);
  useEffect(()=>()=>{metadataRequest.current++;},[]);
  const openColumns=(event:React.MouseEvent<HTMLButtonElement>)=>{
    const rect=event.currentTarget.getBoundingClientRect();
    setColumnsPosition({left:Math.max(12,Math.min(rect.right-330,window.innerWidth-342)),top:Math.min(rect.bottom+6,window.innerHeight-160)});
    setColumns(v=>!v);
    if(columns||!metadataLoader)return;
    const generation=++metadataRequest.current;setReadingMetadata(true);setMetadataError(null);
    void metadataLoader().then(fields=>{if(generation===metadataRequest.current)setMetadataFields(fields);}).catch(error=>{if(generation===metadataRequest.current)setMetadataError(String(error));}).finally(()=>{if(generation===metadataRequest.current)setReadingMetadata(false);});
  };
  const availableColumns=useMemo(()=>Array.from(new Set([...prefs.order,"artist","artwork","album","display_path","state","filetype","library.date_added","library.date_modified",...metadataFields,...rows.flatMap(row=>Object.keys(row.metadata??{}))])),[prefs.order,metadataFields,rows]);
  const toggleColumn=(column:string)=>setPrefs(p=>p.order.includes(column)?{...p,hidden:p.hidden.includes(column)?p.hidden.filter(c=>c!==column):[...p.hidden,column]}:{order:[...p.order,column],hidden:p.hidden.filter(c=>c!==column)});
  const anchor=useRef<{id:string;offset:number}|null>(null),previous=useRef(rows);
  useEffect(()=>{try{localStorage.setItem("track-columns",JSON.stringify({...prefs,version:layoutVersion}));}catch{}},[prefs]);
  useEffect(()=>{if(!menu)return;const close=(event:MouseEvent)=>{if(!(event.target instanceof Element)||!event.target.closest(".context-menu,.row-actions"))setMenu(null);};const key=(event:KeyboardEvent)=>{if(event.key==="Escape")setMenu(null);};document.addEventListener("mousedown",close);document.addEventListener("keydown",key);return()=>{document.removeEventListener("mousedown",close);document.removeEventListener("keydown",key);};},[menu]);
  useLayoutEffect(()=>{
    if(previous.current!==rows&&anchor.current&&viewport.current){const index=rows.findIndex(r=>r.id===anchor.current!.id);if(index>=0){viewport.current.scrollTop=index*height+anchor.current.offset;setTop(viewport.current.scrollTop);}}
    previous.current=rows;
  },[rows]);
  const visibleColumns=prefs.order.filter(c=>!prefs.hidden.includes(c));
  const grid=["27px","29px","28px",...visibleColumns.map(c=>widths[c]??"160px"),"38px"].join(" ");
  const first=Math.max(0,Math.floor(top/height)-8),visible=rows.slice(first,first+48);
  const changeOrder=(column:Column,delta:number)=>setPrefs(p=>{const next=[...p.order],i=next.indexOf(column),n=i+delta;if(n<0||n>=next.length)return p;[next[i],next[n]]=[next[n],next[i]];return {...p,order:next};});
  const sortButton=(c:Sort,text:string)=><button onClick={()=>onSort(c)}>{text}{sort===c?(descending?<ChevronDown size={12}/>:<ChevronUp size={12}/>):null}</button>;
  const head=(c:Column)=>{
    if(c==="title")return <span className="track-head">{sortButton("title","TRACK")}<span aria-hidden="true">/</span>{sortButton("artist","ARTIST")}</span>;
    if(c==="artist"||c==="bpm"||c==="energy")return sortButton(c,label(c).toUpperCase());
    return c==="preview"?"PREVIEW":label(c).toUpperCase();
  };
  const status=(row:Track,job:Job|undefined)=>{
    if(job?.state==="attention")return <span className="tag-status attention" role="img" aria-label="Needs attention" title={job.error??"Needs attention"}><Glyph name="warning" size={16}/></span>;
    if(job)return <span className="tag-status working" role="img" aria-label={stages[job.stage]??"Preparing"} title={stages[job.stage]??"Preparing"}><LoaderCircle size={15} className="spin"/></span>;
    const tag=row.metadata?.["library.tag_status"];
    if(tag==="matched")return <span className="tag-status ok" role="img" aria-label="Tagged from catalog match" title="Tagged from catalog match"><Check size={15} strokeWidth={2.4}/></span>;
    if(tag==="already_tagged")return <span className="tag-status ok" role="img" aria-label="Already tagged" title="Already tagged"><Check size={15} strokeWidth={2.4}/></span>;
    if(tag==="no_match")return <span className="tag-status unmatched" role="img" aria-label="No catalog match; existing tags kept" title="No catalog match; existing tags kept"><Glyph name="tags" size={16}/></span>;
    return <span className="tag-status none" aria-hidden="true"/>;
  };
  const cell=(row:Track,job:Job|undefined,column:Column)=>{
    switch(column){
      case "title":return <div className="title-cell">{!visibleColumns.includes("artwork")?<span className="file-mark"><Artwork id={row.id} identity={row.source_sha256} load={artworkLoader} fallback={<Music2 size={15}/>}/></span>:null}<span className={"title-content"+(view==="incoming"&&job?" has-status":"")}><span className="title-line"><strong title={row.title}>{row.title}</strong><small className="file-extension">{row.display_path.split(".").pop()?.toUpperCase()}</small></span>{view==="incoming"&&job?<small className={job.state==="attention"?"attention":"processing"}>{job.state==="attention"?job.error:stages[job.stage]??job.stage}</small>:<small className="artist-line" title={row.artist}>{row.artist||"—"}</small>}</span></div>;
      case "artwork":return <span className="file-mark"><Artwork id={row.id} identity={row.source_sha256} load={artworkLoader} fallback={<Music2 size={17}/>}/></span>;
      case "artist":return <span className="ellipsis muted">{row.artist||"—"}</span>;
      case "genre":return <span className="ellipsis genre">{row.genre||"—"}</span>;
      case "bpm":return <span className="number">{row.bpm?.toFixed(2)??"—"}</span>;
      case "key":return row.key?<span className="key-chip">{row.key}</span>:<span className="dim">—</span>;
      case "energy":return row.energy!=null?<span className="energy"><span>{row.energy}/10</span><span className="energy-bar" aria-hidden="true"><i style={{width:row.energy*10+"%"}}/></span></span>:<span className="dim">—</span>;
      case "preview":return <RowPreview id={row.id} identity={row.source_sha256} load={previewLoader}/>;
      case "duration":return <span className="number muted">{time(row.duration)}</span>;
      default:{const value=column==="album"?row.album:column==="display_path"?row.display_path:column==="state"?row.state:column==="filetype"?(row.metadata?.["format.format_name"]??row.display_path.split(".").pop()?.toUpperCase()):row.metadata?.[column];const text=metadataText(column,value);return <span className="ellipsis muted" title={text}>{text}</span>;}

    }
  };
  return <div className="table-wrap" role="table" aria-label={view==="incoming"?"Incoming tracks":"Music library"} style={{"--track-columns":grid} as React.CSSProperties}>
    <div className="table-header table-grid" role="row" ref={header}>
      <div role="columnheader"/><div role="columnheader">#</div><div role="columnheader" aria-label="Preparation status"/>{visibleColumns.map(c=><div role="columnheader" key={c}>{head(c)}</div>)}
      <button aria-label="Columns" title="Choose and order columns" onClick={openColumns}><Columns3 size={14}/></button>
    </div>
    {columns?createPortal(<div className="columns-panel" style={columnsPosition} role="dialog" aria-label="Table columns"><div className="columns-title">Table columns<button aria-label="Close columns" onClick={()=>setColumns(false)}><X size={14}/></button></div><input className="column-search" aria-label="Search columns" placeholder="Find metadata…" value={columnSearch} onChange={e=>setColumnSearch(e.target.value)}/>{readingMetadata?<small>Reading embedded metadata…</small>:null}{metadataError?<small role="alert">{metadataError}</small>:null}{availableColumns.filter(c=>label(c).toLowerCase().includes(columnSearch.toLowerCase())).map(c=>{const i=prefs.order.indexOf(c);return <div className="column-option" key={c}><label><input type="checkbox" checked={i>=0&&!prefs.hidden.includes(c)} onChange={()=>toggleColumn(c)}/>{label(c)}</label><button aria-label={"Move "+label(c)+" left"} disabled={i<=0} onClick={()=>changeOrder(c,-1)}><ArrowUp size={12}/></button><button aria-label={"Move "+label(c)+" right"} disabled={i<0||i===prefs.order.length-1} onClick={()=>changeOrder(c,1)}><ArrowDown size={12}/></button></div>;})}</div>,document.body):null}
    <div className="table-body" ref={viewport} onScroll={event=>{const node=event.currentTarget;setTop(node.scrollTop);if(header.current)header.current.scrollLeft=node.scrollLeft;const index=Math.floor(node.scrollTop/height);anchor.current=rows[index]?{id:rows[index].id,offset:node.scrollTop-index*height}:null;if(hasMore&&!loading&&node.scrollTop+node.clientHeight>node.scrollHeight-500)onMore();}}
      onKeyDown={event=>{if(event.key==="ArrowDown"||event.key==="ArrowUp"){event.preventDefault();const index=rows.findIndex(r=>selected.has(r.id)),next=rows[Math.min(rows.length-1,Math.max(0,index+(event.key==="ArrowDown"?1:-1)))];if(next){onSelect(next,{metaKey:false,ctrlKey:false,shiftKey:false} as React.MouseEvent);const y=rows.indexOf(next)*height;viewport.current?.scrollTo({top:Math.max(0,y-100)});}}if(event.key==="Enter"){const row=rows.find(r=>selected.has(r.id));if(row)onPlay(row);}}} tabIndex={0} aria-label="Track list">
      <div style={{height:rows.length*height,position:"relative"}}>
        {visible.map((row,index)=>{const job=jobs.find(j=>j.track_id===row.id&&j.state!=="completed");return <div className={["table-row","table-grid",selected.has(row.id)?"selected":"",playingId===row.id?"playing":""].join(" ")} role="row" key={row.id} aria-selected={selected.has(row.id)} data-track-drop-id={row.id} draggable={false} onDragStart={event=>event.preventDefault()} onPointerDown={event=>{if(!(event.target as Element).closest("button,input,select,a"))onTrackDragStart?.(row,event);}} style={{position:"absolute",top:(first+index)*height,height,width:"100%"}} onClick={e=>onSelect(row,e)} onDoubleClick={()=>onPlay(row)}>
          <div className="row-preview" role="cell"><button aria-label={"Play "+row.title} onClick={e=>{e.stopPropagation();onPlay(row);}}>{playingId===row.id?<span className="equalizer"><i/><i/><i/></span>:<Play size={13}/>}</button></div>
          <div className="row-number" role="cell">{first+index+1}</div><div className="tag-status-cell" role="cell">{status(row,job)}</div>{visibleColumns.map(c=><div className="metadata-cell" role="cell" key={c}>{cell(row,job,c)}</div>)}
          <div className="row-actions" role="cell">{job?.state==="attention"?<button aria-label={"Retry "+row.title} title="Retry preparation" onClick={e=>{e.stopPropagation();onRetry(job.id);}}><RotateCcw size={15}/></button>:<button aria-label={"Actions for "+row.title} onClick={e=>{e.stopPropagation();const rect=e.currentTarget.getBoundingClientRect();setMenu(menu?.id===row.id?null:{id:row.id,x:Math.max(8,rect.right-186),y:Math.min(rect.bottom,window.innerHeight-110)});}}><MoreHorizontal size={17}/></button>}</div>
        </div>;})}
      </div>{loading&&rows.length>0?<div className="loading-more">Loading tracks…</div>:null}
    </div>
    {menu?createPortal(<div className="context-menu" role="menu" style={{position:"fixed",left:menu.x,top:menu.y}}><button role="menuitem" onClick={()=>{onReprepare(menu.id);setMenu(null);}}><RotateCcw size={14}/>Analyze again</button><button role="menuitem" onClick={()=>{onArchive(menu.id);setMenu(null);}}><Archive size={14}/>{view==="archive"?"Restore to library":"Archive track"}</button></div>,document.body):null}
  </div>;
}
