# Run from MSYS2 MinGW64 with gcc, make, nasm, python and curl.
$ErrorActionPreference = "Stop"
bash tools/media-runtime/build.sh
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
