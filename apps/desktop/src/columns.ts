const labels:Record<string,string>={title:"Title",artist:"Artist",album:"Album",genre:"Genre",bpm:"BPM",key:"Key",energy:"Energy",duration:"Duration",display_path:"Location",state:"Status",artwork:"Album art",filetype:"Filetype","library.date_added":"Date added","library.date_modified":"Date modified","audio.codec":"Codec","audio.sample_rate":"Sample rate","audio.channels":"Channels","audio.bits_per_sample":"Bit depth","format.bit_rate":"Bitrate","format.size":"Size","format.duration":"Encoded duration","format.format_name":"Container"};
const tags:Record<string,string>={album_artist:"Album artist",albumartist:"Album artist",comment:"Comment",description:"Description",rating:"Rating",year:"Year",date:"Release date",originaldate:"Original release date",track:"Track No.",tracknumber:"Track No.",disc:"Disc No.",discnumber:"Disc No.",grouping:"Grouping",lyricist:"Lyricist",remixer:"Remixer",mix:"Mix",version:"Mix",label:"Label",publisher:"Publisher",composer:"Composer",producer:"Producer",isrc:"ISRC",catalog_number:"Catalog number",catalognumber:"Catalog number",initialkey:"Embedded key",initial_key:"Embedded key",key:"Embedded key",bpm:"Embedded BPM",title:"Embedded title",artist:"Embedded artist",album:"Embedded album",genre:"Embedded genre"};
export function columnLabel(id:string):string {
  if(Object.hasOwn(labels,id))return labels[id];
  const bare=id.replace(/^(tag|audio|format)\./,"");
  return (id.startsWith("tag.")&&Object.hasOwn(tags,bare)?tags[bare]:null)??bare.replace(/[_.]/g," ").replace(/^./,s=>s.toUpperCase());
}
export function metadataText(id:string,value:unknown):string {
  if(value==null||value==="")return "—";
  if(id.startsWith("library.date_")){const d=new Date(Number(value));return Number.isFinite(d.getTime())?d.toLocaleDateString():"—";}
  if(id==="format.bit_rate"&&Number.isFinite(Number(value)))return (Number(value)/1000).toFixed(0)+" kbps";
  if(id==="format.size"&&Number.isFinite(Number(value)))return (Number(value)/1048576).toFixed(1)+" MB";
  return typeof value==="object"?JSON.stringify(value):String(value);
}
