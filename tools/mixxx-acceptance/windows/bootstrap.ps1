# Per-user setup for the Mixxx acceptance run on Windows. Needs no administrator rights: every
# tool is unpacked under $Root and put on PATH by $Root\env.ps1, nothing is installed system-wide.
#
#   curl.exe -sSL -o bootstrap.ps1 https://raw.githubusercontent.com/johnneerdael/CueRithm/<branch>/tools/mixxx-acceptance/windows/bootstrap.ps1
#   powershell -ExecutionPolicy Bypass -File bootstrap.ps1
#
# What it fetches (all from the projects' own release pages; Mixxx's installer is checked against
# the SHA-256 the Mixxx download server publishes next to it):
#   PortableGit, WinLibs (GCC and the MinGW-w64 headers and import libraries, which include Media
#   Foundation), Strawberry Perl (rusqlite's SQLCipher feature builds OpenSSL from source), Rust
#   (GNU host, because the Visual Studio Build Tools need administrator rights), the embeddable
#   Python, a static FFmpeg, and Mixxx's own Windows installer unpacked as an administrative
#   image (no installation).
param(
    [string]$Root = 'D:\cuerithm-win',
    [string]$Mixxx = '2.5.6',
    [string]$Python = '3.12.10'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
New-Item -ItemType Directory -Force -Path $Root, "$Root\dl" | Out-Null

function Fetch([string]$url, [string]$out) {
    if (Test-Path $out) { return }
    Write-Host "download $url"
    curl.exe -sSL --fail -o $out $url
    if ($LASTEXITCODE) { Remove-Item -Force -ErrorAction SilentlyContinue $out; throw "download failed: $url" }
}
function LatestAsset([string]$repo, [string]$pattern) {
    $json = curl.exe -sSL --fail "https://api.github.com/repos/$repo/releases/latest" | Out-String
    $asset = ($json | ConvertFrom-Json).assets | Where-Object { $_.name -match $pattern } | Select-Object -First 1
    if (-not $asset) { throw "no release asset of $repo matches $pattern" }
    $asset
}

# 1. Git
if (-not (Test-Path "$Root\git\cmd\git.exe")) {
    $a = LatestAsset 'git-for-windows/git' '^PortableGit-.*-64-bit\.7z\.exe$'
    Fetch $a.browser_download_url "$Root\dl\$($a.name)"
    & "$Root\dl\$($a.name)" -o"$Root\git" -y | Out-Null
}

# 2. GCC with the MinGW-w64 headers and import libraries (Media Foundation among them)
if (-not (Test-Path "$Root\mingw64\bin\gcc.exe")) {
    $a = LatestAsset 'brechtsanders/winlibs_mingw' '^winlibs-x86_64-posix-seh-gcc-[\d.]+-mingw-w64ucrt-[\d.]+-r\d+\.zip$'
    Fetch $a.browser_download_url "$Root\dl\$($a.name)"
    tar.exe -xf "$Root\dl\$($a.name)" -C $Root
}

# 2b. Perl and make, for the OpenSSL that rusqlite's SQLCipher feature builds from source
if (-not (Test-Path "$Root\perl\perl\bin\perl.exe")) {
    $a = LatestAsset 'StrawberryPerl/Perl-Dist-Strawberry' '^strawberry-perl-[\d.]+-64bit-portable\.zip$'
    Fetch $a.browser_download_url "$Root\dl\$($a.name)"
    New-Item -ItemType Directory -Force "$Root\perl" | Out-Null
    tar.exe -xf "$Root\dl\$($a.name)" -C "$Root\perl"
}
New-Item -ItemType Directory -Force "$Root\bin" | Out-Null
if (-not (Test-Path "$Root\bin\make.exe")) {
    Copy-Item "$Root\mingw64\bin\mingw32-make.exe" "$Root\bin\make.exe"
}

# 3. Rust, GNU host
$env:CARGO_HOME = "$Root\cargo"
$env:RUSTUP_HOME = "$Root\rustup"
if (-not (Test-Path "$Root\cargo\bin\cargo.exe")) {
    Fetch 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe' "$Root\dl\rustup-init.exe"
    & "$Root\dl\rustup-init.exe" -y --default-host x86_64-pc-windows-gnu --default-toolchain stable `
        --profile minimal --no-modify-path | Out-Host
}

# 4. Python (embeddable: it only runs scripts, which is all the harness needs)
if (-not (Test-Path "$Root\python\python.exe")) {
    Fetch "https://www.python.org/ftp/python/$Python/python-$Python-embed-amd64.zip" "$Root\dl\python.zip"
    Expand-Archive -Force "$Root\dl\python.zip" "$Root\python"
}

# 5. FFmpeg, to make the test audio and to read where a decoder starts
if (-not (Test-Path "$Root\ffmpeg\bin\ffmpeg.exe")) {
    Fetch 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip' "$Root\dl\ffmpeg.zip"
    tar.exe -xf "$Root\dl\ffmpeg.zip" -C $Root
    $unpacked = Get-ChildItem $Root -Directory | Where-Object Name -like 'ffmpeg-master-latest-win64-gpl*' | Select-Object -First 1
    Rename-Item $unpacked.FullName "$Root\ffmpeg"
}

# 6. Mixxx, from its own download server, unpacked without installing
$base = "https://downloads.mixxx.org/releases/$Mixxx"
$msi = "$Root\dl\mixxx-$Mixxx-win64.msi"
Fetch "$base/mixxx-$Mixxx-win64.msi" $msi
Fetch "$base/mixxx-$Mixxx-win64.msi.sha256sum" "$msi.sha256sum"
$wanted = ((Get-Content "$msi.sha256sum" -Raw).Trim() -split '\s+')[0].ToLower()
$actual = (Get-FileHash -Algorithm SHA256 $msi).Hash.ToLower()
if ($wanted -ne $actual) { throw "Mixxx installer checksum differs: wanted $wanted, got $actual" }
Write-Host "Mixxx $Mixxx installer checksum matches ($actual)"
if (-not (Get-ChildItem "$Root\mixxx-$Mixxx" -Recurse -Filter mixxx.exe -ErrorAction SilentlyContinue)) {
    Start-Process msiexec.exe -Wait -ArgumentList '/a', "`"$msi`"", '/qn', "TARGETDIR=`"$Root\mixxx-$Mixxx`""
}

# The environment later scripts dot-source
@"
`$Root = '$Root'
`$env:CARGO_HOME = "`$Root\cargo"
`$env:RUSTUP_HOME = "`$Root\rustup"
`$env:PATH = "`$Root\git\cmd;`$Root\mingw64\bin;`$Root\bin;`$Root\perl\perl\bin;`$Root\cargo\bin;`$Root\python;`$Root\ffmpeg\bin;`$env:PATH"
"@ | Set-Content -Encoding ASCII "$Root\env.ps1"

. "$Root\env.ps1"
"--- versions"
git --version
(gcc --version)[0]
cargo --version
rustc --version --verbose | Select-String 'host'
python --version
(ffmpeg -version)[0]
$exe = Get-ChildItem "$Root\mixxx-$Mixxx" -Recurse -Filter mixxx.exe | Select-Object -First 1
"mixxx.exe: $($exe.FullName)"
"SETUP COMPLETE"
