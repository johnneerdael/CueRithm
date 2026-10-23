import {describe,expect,it} from "vitest";
import {blockerLabel,conflictLabel,providerLabel} from "../catalog-labels";

describe("catalog labels",()=>{
  it("names every conflict code the matcher can report",()=>{
    expect(["edition","isrc","album","release_id","duration","recording_id","release_track_id","release_edition_unresolved"].map(conflictLabel)).toEqual([
      "Edition differs","ISRC differs","Album differs","Release ID differs","Duration differs","Recording ID differs","Release track ID differs","Release edition could not be resolved"]);
  });
  it("names every provider id, whatever its case",()=>{
    expect(["beatport","traxsource","musicbrainz","sonovault","discogs","volumo","itunes","bpmsupreme","bandcamp","deezer"].map(providerLabel)).toEqual([
      "Beatport","Traxsource","MusicBrainz","SonoVault","Discogs","Volumo","iTunes","BPM Supreme","Bandcamp","Deezer"]);
    expect([providerLabel("Beatport"),providerLabel(" TRAXSOURCE "),providerLabel("bpm_supreme")]).toEqual(["Beatport","Traxsource","BPM Supreme"]);
  });
  it("turns an unknown code into a readable phrase and leaves prose alone",()=>{
    expect(conflictLabel("label_mismatch")).toBe("Label mismatch");
    expect(conflictLabel("Version differs: Club Mix / Dub Mix")).toBe("Version differs: Club Mix / Dub Mix");
    expect(providerLabel("newstore")).toBe("Newstore");
    expect(conflictLabel("")).toBe("Unknown");
    expect(providerLabel("  ")).toBe("Unknown");
  });
  it("never reads inherited object keys as known codes",()=>{
    for(const code of ["constructor","__proto__","toString","hasOwnProperty"]){
      expect(typeof conflictLabel(code)).toBe("string");
      expect(typeof providerLabel(code)).toBe("string");
    }
    expect(conflictLabel("constructor")).toBe("Constructor");
    expect(providerLabel("__proto__")).toBe("Proto");
  });
  it("labels the codes inside the acknowledge blocker and passes other blockers through",()=>{
    expect(blockerLabel("Explicitly acknowledge the selected recording differences: edition, release_id")).toBe("Explicitly acknowledge the selected recording differences: Edition differs, Release ID differs");
    expect(blockerLabel("Choose a field")).toBe("Choose a field");
  });
});
