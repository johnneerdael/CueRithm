# Per-user setup for the Mixxx acceptance run on Windows. Needs no administrator rights: every
# tool is unpacked under $Root and put on PATH by $Root\env.ps1, nothing is installed system-wide.
#
#   curl.exe -sSL -o bootstrap.ps1 https://raw.githubusercontent.com/johnneerdael/CueRithm/<branch>/tools/mixxx-acceptance/windows/bootstrap.ps1
#   powershell -ExecutionPolicy Bypass -File bootstrap.ps1
#
# What it fetches (all from the projects' own release pages; Mixxx's installer is checked against
# the SHA-256 the Mixxx download server publishes next to it):
#   PortableGit (its MSYS Perl and sh build the OpenSSL that rusqlite's SQLCipher feature compiles
#   from source), WinLibs (GCC and the MinGW-w64 headers and import libraries, which include Media
#   Foundation), Rust (GNU host, because the Visual Studio Build Tools need administrator rights),
#   the embeddable Python, a static FFmpeg, and Mixxx's own Windows installer unpacked as an
#   administrative image (no installation).
#
# -Toolchain msvc is for a machine that already has the Visual Studio Build Tools and a Rust with
# the x86_64-pc-windows-msvc host (a GitHub-hosted windows runner; .github/workflows/mixxx-windows.yml).
# That is the toolchain the shipped Windows app is built with. It fetches no Git and no Rust, and
# leaves Perl alone (the runner's Strawberry Perl builds OpenSSL for the MSVC target; the MinGW
# folder goes last on PATH so that only the probes and libmad, which are built with GCC, find it).
param(
    [string]$Root = 'D:\cuerithm-win',
    [string]$Mixxx = '2.5.6',
    [string]$Python = '3.12.10',
    [ValidateSet('gnu', 'msvc')][string]$Toolchain = 'gnu'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
New-Item -ItemType Directory -Force -Path $Root, "$Root\dl" | Out-Null

function Fetch([string]$url, [string]$out) {
    if (Test-Path $out) { return }
    Write-Host "download $url"
    # to a temporary name first: an interrupted download must not look finished
    curl.exe -sSL --fail -o "$out.part" $url
    if ($LASTEXITCODE) { Remove-Item -Force -ErrorAction SilentlyContinue "$out.part"; throw "download failed: $url" }
    Move-Item -Force "$out.part" $out
}
function LatestAsset([string]$repo, [string]$pattern) {
    # A hosted runner shares its address with many others, so the anonymous API limit runs out;
    # a token (the workflow passes its own) lifts it. It goes to api.github.com only.
    $auth = @()
    if ($env:GITHUB_TOKEN) { $auth = @('-H', "Authorization: Bearer $($env:GITHUB_TOKEN)") }
    $json = curl.exe -sSL --fail @auth "https://api.github.com/repos/$repo/releases/latest" | Out-String
    if ($LASTEXITCODE) { throw "release lookup failed: $repo" }
    $asset = ($json | ConvertFrom-Json).assets | Where-Object { $_.name -match $pattern } | Select-Object -First 1
    if (-not $asset) { throw "no release asset of $repo matches $pattern" }
    $asset
}

# 1. Git (the msvc toolchain runs where Git is already installed)
if ($Toolchain -eq 'gnu' -and -not (Test-Path "$Root\git\cmd\git.exe")) {
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

New-Item -ItemType Directory -Force "$Root\bin" | Out-Null
if ($Toolchain -eq 'gnu') {
    # 2b. Perl, sh and make, for the OpenSSL that rusqlite's SQLCipher feature builds from source.
    # OpenSSL's MinGW target refuses a native Windows Perl (Strawberry Perl "doesn't produce Unix like
    # paths"); the MSYS Perl and sh inside Git for Windows do, so their folder goes last on PATH (the
    # Windows tools keep priority), and make is the MinGW one under the name OpenSSL looks for. Git's
    # Perl is a cut-down one without the core module Locale::Maketext::Simple, which OpenSSL's Configure
    # loads (through IPC::Cmd), so that one pure-Perl file is fetched from CPAN.
    if (-not (Test-Path "$Root\git\usr\bin\perl.exe")) { throw "PortableGit has no usr\bin\perl.exe" }
    $perlLib = "$Root\git\usr\share\perl5\core_perl"
    if (-not (Test-Path "$perlLib\Locale\Maketext\Simple.pm")) {
        $name = 'Locale-Maketext-Simple-0.21'
        Fetch "https://cpan.metacpan.org/authors/id/J/JE/JESSE/$name.tar.gz" "$Root\dl\$name.tar.gz"
        tar.exe -xzf "$Root\dl\$name.tar.gz" -C "$Root\dl"
        New-Item -ItemType Directory -Force "$perlLib\Locale\Maketext" | Out-Null
        Copy-Item "$Root\dl\$name\lib\Locale\Maketext\Simple.pm" "$perlLib\Locale\Maketext\Simple.pm"
    }
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
} else {
    # 3. Rust and the Visual Studio C++ tools must already be there; say so rather than build with
    # something else and report it as MSVC
    $rustHost = @(rustc --version --verbose | Where-Object { $_ -like 'host:*' })[0]
    if ($rustHost -ne 'host: x86_64-pc-windows-msvc') {
        throw "the msvc toolchain needs a Rust with the x86_64-pc-windows-msvc host, found: $rustHost"
    }
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

# 7. libmad, the MP3 decoder Mixxx for Windows uses (its log names "MAD: MPEG Audio Decoder
# 0.16.4 NDEBUG FPM_64BIT OPT_ACCURACY"), built the way that line says from the maintained
# 0.16.4 source so that `mad_probe.c` can read where it starts an MP3. CMake comes as a portable
# zip.
if (-not (Test-Path "$Root\libmad\lib\libmad.a")) {
    $cmakeVersion = '3.30.5'
    if (-not (Test-Path "$Root\cmake\bin\cmake.exe")) {
        Fetch "https://github.com/Kitware/CMake/releases/download/v$cmakeVersion/cmake-$cmakeVersion-windows-x86_64.zip" "$Root\dl\cmake.zip"
        tar.exe -xf "$Root\dl\cmake.zip" -C $Root
        Rename-Item "$Root\cmake-$cmakeVersion-windows-x86_64" "$Root\cmake"
    }
    Fetch 'https://codeberg.org/tenacityteam/libmad/archive/0.16.4.zip' "$Root\dl\libmad-0.16.4.zip"
    if (-not (Test-Path "$Root\libmad-src\CMakeLists.txt")) {
        tar.exe -xf "$Root\dl\libmad-0.16.4.zip" -C $Root
        Rename-Item "$Root\libmad" "$Root\libmad-src"
    }
    $env:PATH = "$Root\cmake\bin;$Root\mingw64\bin;$env:PATH"
    # CMake writes progress to stderr, which PowerShell 5.1 turns into a terminating error while
    # the preference is Stop (the script then ends without a word): judge the exit codes instead
    $ErrorActionPreference = 'Continue'
    # (CMAKE_SH: a sh.exe on PATH, as Git for Windows puts one on a hosted runner, makes the MinGW
    # generator refuse to run)
    cmake -S "$Root\libmad-src" -B "$Root\libmad-build" -G 'MinGW Makefiles' -DCMAKE_SH=CMAKE_SH-NOTFOUND `
        -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DOPTIMIZE=ACCURACY -DEXAMPLE=OFF `
        "-DCMAKE_INSTALL_PREFIX=$Root\libmad" 2>&1 | Out-Host
    $configured = $LASTEXITCODE
    cmake --build "$Root\libmad-build" --target install 2>&1 | Out-Host
    $built = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($configured) { throw 'libmad: cmake configure failed' }
    if ($built) { throw 'libmad: build failed' }
}

# The environment later scripts dot-source
if ($Toolchain -eq 'gnu') {
@"
`$Root = '$Root'
`$env:CARGO_HOME = "`$Root\cargo"
`$env:RUSTUP_HOME = "`$Root\rustup"
`$env:CUERITHM_LIBMAD = "`$Root\libmad"
`$env:PATH = "`$Root\git\cmd;`$Root\mingw64\bin;`$Root\bin;`$Root\cargo\bin;`$Root\python;`$Root\ffmpeg\bin;`$env:PATH;`$Root\git\usr\bin"
"@ | Set-Content -Encoding ASCII "$Root\env.ps1"
} else {
    # cargo and rustc stay the machine's own (the msvc host); MinGW only has to provide the gcc of the probes
@"
`$Root = '$Root'
`$env:CUERITHM_LIBMAD = "`$Root\libmad"
`$env:PATH = "`$Root\mingw64\bin;`$Root\bin;`$Root\python;`$Root\ffmpeg\bin;`$env:PATH"
"@ | Set-Content -Encoding ASCII "$Root\env.ps1"
}

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
