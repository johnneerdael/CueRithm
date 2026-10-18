import library from "../brand/icons/library.svg";
import incoming from "../brand/icons/incoming.svg";
import folder from "../brand/icons/folder.svg";
import cleanup from "../brand/icons/cleanup.svg";
import sync from "../brand/icons/sync.svg";
import settings from "../brand/icons/settings.svg";
import tags from "../brand/icons/tags.svg";
import warning from "../brand/icons/warning.svg";
import smartPlaylist from "../brand/icons/smart-playlist.svg";
const glyphs={library,incoming,folder,cleanup,sync,settings,tags,warning,smartPlaylist};
export type GlyphName=keyof typeof glyphs;
// The kit's SVGs cannot inherit CSS colour as <img>, so they are used as masks over currentColor.
export default function Glyph({name,size=18}:{name:GlyphName;size?:number}){
  const mask=`url("${glyphs[name]}") center/contain no-repeat`;
  return <span className="glyph" aria-hidden="true" style={{width:size,height:size,WebkitMask:mask,mask}}/>;
}
