# Native ASF metadata source

TagLib 2.3.2: https://github.com/taglib/taglib/releases/tag/v2.3.2

`manifest.json` pins the original release and the prepared build-source archive.
`prepare.py ORIGINAL.tar.gz taglib-2.3.2-sources.tar.gz` reproduces the selection.
Tests/examples/benchmarks/CI/audio fixtures are omitted. Source code and notices
needed to build the static library, including bundled utf8cpp headers, remain.

The Rust build script verifies the prepared checksum and compiles ASF support
without network access or external zlib/utf8cpp discovery. Other optional format
families, shared libraries, tests, examples and C bindings are disabled. The
small C++ bridge exposes only staged-file ASF metadata operations. Other formats
use the selected OneTagger writers. CMake and a C++17 compiler are build tools;
they are not end-user runtime requirements. The independent analysis engine
remains separately MIT-licensed; TagLib retains its LGPL/MPL notices.

Cuerithm's build script applies one checked, idempotent ASF modification to
`taglib/asf/asffile.cpp`: repeated values are serialized together in the Metadata
Library object. Upstream splits the first value into a different header object;
when an existing file orders those objects differently, a save/read can reorder
contributors. Scalar attributes retain upstream placement. This modification is
covered by the generated-audio metadata/PCM round-trip tests and retains TagLib's
LGPL/MPL licensing. The pinned source archive itself is unchanged.
