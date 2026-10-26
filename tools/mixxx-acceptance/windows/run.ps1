# Runs acceptance tasks on Windows with everything on PATH, logging each to $Root\<task>.log.
#
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task tests        # cuerithm-dj-sync unit tests
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task acceptance   # run.py --windows against Mixxx
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task all          # the two, one after the other
#
# Start it hidden and read the log, so a long build does not depend on a console staying open:
#   Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"$Root\CueRithm\tools\mixxx-acceptance\windows\run.ps1",'-Task','all'
#   Get-Content $Root\tests.log -Tail 20
#
# -Repo is the checkout to test (default $Root\CueRithm); -Work keeps the acceptance run's files
# (Mixxx's logs and libraries, the report) in that folder instead of a temporary one. The exit status
# is 1 when a task failed, which is what .github/workflows/mixxx-windows.yml relies on.
param(
    [ValidateSet('tests', 'acceptance', 'all')][string]$Task = 'tests',
    [string]$Root = 'D:\cuerithm-win',
    [string]$Repo = '',
    [string]$Work = ''
)
. "$Root\env.ps1"
if (-not $Repo) { $Repo = "$Root\CueRithm" }
Set-Location $Repo
$names = if ($Task -eq 'all') { @('tests', 'acceptance') } else { @($Task) }
$failed = 0
foreach ($name in $names) {
    $log = "$Root\$name.log"
    $command = if ($name -eq 'tests') {
        'cargo test -p cuerithm-dj-sync --lib'
    } else {
        $line = 'python tools\mixxx-acceptance\run.py --windows'
        if ($Work) { $line += " --work `"$Work`"" }
        $line
    }
    "started $(Get-Date -Format s): $command" | Set-Content $log
    # cmd does the redirection: PowerShell 5.1 would wrap a native command's stderr lines as errors
    cmd /c "$command >> `"$log`" 2>&1"
    if ($LASTEXITCODE) { $failed = 1 }
    "exit: $LASTEXITCODE at $(Get-Date -Format s)" | Add-Content $log
}
exit $failed
