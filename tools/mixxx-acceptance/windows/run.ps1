# Runs acceptance tasks on Windows with everything on PATH, logging each to $Root\<task>.log.
#
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task tests        # cuerithm-dj-sync unit tests
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task acceptance   # run.py --windows against Mixxx
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task all          # the two, one after the other
#
# Start it hidden and read the log, so a long build does not depend on a console staying open:
#   Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"$Root\CueRithm\tools\mixxx-acceptance\windows\run.ps1",'-Task','all'
#   Get-Content $Root\tests.log -Tail 20
param(
    [ValidateSet('tests', 'acceptance', 'all')][string]$Task = 'tests',
    [string]$Root = 'D:\cuerithm-win'
)
. "$Root\env.ps1"
Set-Location "$Root\CueRithm"
$names = if ($Task -eq 'all') { @('tests', 'acceptance') } else { @($Task) }
foreach ($name in $names) {
    $log = "$Root\$name.log"
    $command = if ($name -eq 'tests') {
        'cargo test -p cuerithm-dj-sync --lib'
    } else {
        'python tools\mixxx-acceptance\run.py --windows'
    }
    "started $(Get-Date -Format s): $command" | Set-Content $log
    # cmd does the redirection: PowerShell 5.1 would wrap a native command's stderr lines as errors
    cmd /c "$command >> `"$log`" 2>&1"
    "exit: $LASTEXITCODE at $(Get-Date -Format s)" | Add-Content $log
}
