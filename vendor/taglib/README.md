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
