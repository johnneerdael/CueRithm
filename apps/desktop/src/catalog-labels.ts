// Plain names for the ids the matcher reports. Conflict codes come from identity_conflicts() and
// match_identity_conflicts() in crates/cuerithm-autotag/src/evidence.rs; provider ids from ProviderId
// in crates/cuerithm-autotag/src/lib.rs (the display names match providers.rs).
const conflicts=new Map<string,string>([
  ["edition","Edition differs"],["isrc","ISRC differs"],["album","Album differs"],["release_id","Release ID differs"],["duration","Duration differs"],
  ["recording_id","Recording ID differs"],["release_track_id","Release track ID differs"],["release_edition_unresolved","Release edition could not be resolved"],
]);
const providers=new Map<string,string>([
  ["beatport","Beatport"],["traxsource","Traxsource"],["musicbrainz","MusicBrainz"],["sonovault","SonoVault"],["discogs","Discogs"],
  ["volumo","Volumo"],["itunes","iTunes"],["bpmsupreme","BPM Supreme"],["bpm_supreme","BPM Supreme"],["bandcamp","Bandcamp"],["deezer","Deezer"],
]);
// A code this build does not know still reads as a sentence. Text that is already prose is left alone.
const humanise=(code:string)=>{const words=code.replaceAll("_"," ").trim();return words?words[0].toUpperCase()+words.slice(1):"Unknown";};
export const conflictLabel=(code:string)=>conflicts.get(code)??humanise(code);
export const providerLabel=(id:string)=>providers.get(id.trim().toLowerCase())??humanise(id);
// The apply step refuses an unacknowledged conflict with this sentence plus the raw codes (workflow.rs in
// cuerithm-library); any other blocker, or a reworded sentence, is shown as it arrives.
const acknowledgement="Explicitly acknowledge the selected recording differences: ";
export const blockerLabel=(blocker:string)=>blocker.startsWith(acknowledgement)?acknowledgement+blocker.slice(acknowledgement.length).split(", ").map(conflictLabel).join(", "):blocker;
