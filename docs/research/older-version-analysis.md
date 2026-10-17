# Older-version analysis paths

Static inspection on 2026-10-17, using the user-supplied macOS10.3.3 and
Windows8.5.2325.0 packages. No supplied application or installer was executed,
modified or used as a product dependency. Reconstructed vendor code and binaries
remain private. These findings do not change the exact-output acceptance gate.

## macOS10.3.3.6836

The ARM64 executable's `MIKAnalysisServiceClient` method at `0x10004c0c4`
serializes local key/energy features, tempo, downbeat and cue scores, constructs
an HTTP POST to the same `AnalyzeSong10` endpoint observed in version11, and
parses `KeySummary`, `Segments`, `EnergyLevelSegments`, `CuePoints` and
`OverallEnergy` from the response. Its analyzed caller is
`MIKAnalysisOperation::performSinglePassSingleSegmentOnnxAnalysis`.
Energy and cue wire versions are `2` and `3` on this method's path.

This establishes a cloud-backed analysis path in the supplied version10 build.
It does not establish every possible mode or the behavior of all10.x releases.
No live network exchange was performed in version10.

REA's full dossier exceeded the MCP transport budget; it was retained and
consumed through complete pseudocode and caller views, without rerunning it.
Parent Evidence `ev_a5ccfbe7a7f523fa3700a1284081d95b8a84a68659af87a70b36153052f7d2cd`;
views `ev_bf81b2ce76378a7d27338ae4b7f398320ae17831107f184c03514895d45f9a34`
and `ev_0311d1751f68c0eed170ba099ba63d71b72fad2965e7eea32f40cc92a25306a8`.
The private export retains all four version10 Evidence records.

## Windows8.5.2325.0

The application assembly identifies itself as8.5.2325.0; the8.0.2325.0 value
reported by the outer setup executable is the bootstrapper version. The WiX
Burn setup contains two cabinets. The second cabinet's size and SHA1 match the
Burn manifest, and all three extracted payload hashes match their manifest
entries. Its MSI yields221 application files, including62 PE images.

The front end is an x86 .NET Framework4.5.2 CIL application. Its analysis path
contains these stages:

1. `AudioAnalyser` uses local native filters for audio features, timing and
   energy data, including `MagellanAnalysisFilter.dll` and `EnergyAnalysis.ax`.
2. `OnlineServicesModule` registers `AnalysisServiceClient` with the configured
   `https://online5.mixedinkey.com` base plus `/Services/Analysis`.
3. `KeyDetectionService.DetectKey` builds `SongAnalysisRequest`, including key
   features, energy features, timing and packed cue scores. `DetectKeyAsync`
   invokes the client's `BeginAnalyzeSong`/`EndAnalyzeSong` network operations.
4. The detection method retries failures and reports a server-connection error.
   No local final-classification fallback appears in that traced method.
5. `StructureAnalysisService` consumes returned key, energy and cue results.

The package also supplies a second executable. The selected analysis client,
key-detection service and online-module classes decompile byte-for-byte
identically in both supplied executables. This comparison concerns those three
reconstructed classes; it does not claim that the complete binaries are equal
or establish every initialization path.

REA assembly metadata Evidence:
`ev_79b21dab574bb8e7cb9f246b01795070bf83633aae977022e52340b00a8c1f31`
and `ev_507507ca706d0da2d44c730d6861aec7a229587e26a902b2a9344903e11c089c`.
The complete managed-member operation hit REA's64MiB representation limit and
reported unavailable coverage. Selected CIL types were instead reconstructed
with the separately installed ILSpy11.1.0.9782, with commands, tool version and
input/output hashes retained privately. That reconstruction is separate from
REA's canonical metadata inspection and is not original source recovery.
Missing .NET Framework reference assemblies leave explicit decompiler warnings.

The supplied executables have a traced remote-analysis path. Fresh-file offline
classification on Windows has not been executed or established. This check
has not recovered local copies of the final service classifiers or proved
that every older release requires the same service.

## Implication for the independent engine

These particular older packages have not yet supplied the missing final
key/energy/cue decision recipe. Their client-side code may still corroborate
individual DSP and protocol operations. Keep that evidence separate from final
classifier recovery and from measured macOS/Linux engine agreement.

## Windows2.5.0.0: local final-key path found

The subsequently supplied2.5 installer contains an x86 mixed CIL/native
application and a bundled `dlltONaRT.dll`. Unlike the traced newer paths, the
fresh-file key path computes its final key locally:

1. The application checks its local Songs table and takes the analysis branch
   for a previously absent file. It opens local audio and reads floating PCM
   with `sf_readf_float`, in blocks of at most16384 frames.
2. Its managed wrapper imports `CtONaRT_If::CreateInstance` directly from the
   bundled Tonart DLL. It feeds PCM to virtual slot+4 (`Process`) and requests
   virtual slot+12 (`GetOverallResults`). `StaticLibWrapper.dll` serves a
   separate tempo path.
3. The wrapper copies a native integer key at result offset1024 and two floats
   at1028/1032. The latter become stored probability and standard-pitch fields;
   they are not energy ratings.
4. A24-entry conversion maps the native major/minor key integer to Camelot.
   The application writes `Songs.Part1_CamelotKey`, which its grid binds to the
   visible Song Key column. This trace covers the fresh-analysis branch,
   separately from previously cached results.

The native constructor's vtable at`0x1007c170` resolves Process to`0x10002e28`
and OverallResults to`0x100030c8`. Further static analysis identifies a local
classifier, not merely an output serializer: it normalizes a12-value tonal
vector, evaluates transposed major/minor profile scores, retains current and
historical scores, and votes among selected candidates to produce the returned
key. The DLL embeds four pairs of12-value major/minor profiles. A full port of
the PCM-to-tonal frontend and numerical/runtime parity remain unfinished.

The original managed image has a malformed custom-attribute coded index, which
blocks ILSpy's normal type loading. An independent metadata-only helper decoded
the original CIL and import trampolines without repairing, loading or executing
the target. REA's native auto-analysis left three proven vtable entries without
functions; a separate Ghidra headless project explicitly defined those entries
and exported their pseudocode/assembly from unchanged DLL bytes. These are
supplemental static reconstructions with the original hashes retained.

The original managed SHA256 is
`0ab89abb24c0d81d63cc9a86adfa01ceee654726bb59b2cc6e0e3a84f662d614`;
the Tonart DLL is
`ea77695193a923702eb2f6ee9276014313ad6126cc7abfa78ca26306c359d17c`.
REA vtable Evidence
`ev_be591152e7d0738a70b4cfcf70106c54cfe1ffb9afd5a46e76ca041679674345`,
constructor Evidence
`ev_f462925262e74ea6d54417f4fe511b642a37e9eac2da20f10feaeca1d3b1188b`.
The independent caller receipt binds29 files and19 original method bodies.

This establishes an inspectable **static local final-key implementation**.
Fresh-track offline execution on Windows has not been performed, and equality
with version11's keys has not been measured. Modern energy ratings and cue
selection were not established by this focused investigation. The independent
engine still needs implementation and measured parity before adopting this
older classifier; bundled vendor binaries are not product dependencies.
