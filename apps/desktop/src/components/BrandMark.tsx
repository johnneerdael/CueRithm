import markLight from "../brand/cuerithm-mark-on-light.svg";
import markDark from "../brand/cuerithm-mark-on-dark.svg";
// Placeholder for tracks without cover art; the theme picks the matching mark.
export default function BrandMark(){
  return <span className="brand-art" aria-hidden="true"><img className="mark-light" src={markLight} alt="" draggable={false}/><img className="mark-dark" src={markDark} alt="" draggable={false}/></span>;
}
