# Runs acceptance tasks on Windows with everything on PATH, logging each to $Root\<task>.log.
#
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task tests        # cuerithm-dj-sync unit tests
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task acceptance   # run.py --windows against Mixxx
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task trash        # the ignored Recycle Bin test, then the bin's contents
#   powershell -ExecutionPolicy Bypass -File run.ps1 -Task all          # tests and acceptance, one after the other
#
# Start it hidden and read the log, so a long build does not depend on a console staying open:
#   Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"$Root\CueRithm\tools\mixxx-acceptance\windows\run.ps1",'-Task','all'
#   Get-Content $Root\tests.log -Tail 20
param(
    [ValidateSet('tests', 'acceptance', 'trash', 'all')][string]$Task = 'tests',
    [string]$Root = 'D:\cuerithm-win'
)
. "$Root\env.ps1"
Set-Location "$Root\CueRithm"
$names = if ($Task -eq 'all') { @('tests', 'acceptance') } else { @($Task) }
foreach ($name in $names) {
    $log = "$Root\$name.log"
    $command = switch ($name) {
        'tests' { 'cargo test -p cuerithm-dj-sync --lib' }
        # Moves one generated file into the real Recycle Bin: the test that the unused-file removal relies on
        'trash' { 'cargo test -p cuerithm-library --lib system_trash -- --ignored --nocapture' }
        default { 'python tools\mixxx-acceptance\run.py --windows' }
    }
    "started $(Get-Date -Format s): $command" | Set-Content $log
    # cmd does the redirection: PowerShell 5.1 would wrap a native command's stderr lines as errors
    cmd /c "$command >> `"$log`" 2>&1"
    "exit: $LASTEXITCODE at $(Get-Date -Format s)" | Add-Content $log
    if ($name -eq 'trash') {
        # The test only proves the file left its folder; this shows it is in the Recycle Bin.
        $kept = @((New-Object -ComObject Shell.Application).NameSpace(10).Items() | Where-Object { $_.Name -like 'cuerithm-trash-check*' })
        "recycle bin entries named cuerithm-trash-check*: $($kept.Count)" | Add-Content $log
        $kept | ForEach-Object { "  $($_.Name)" | Add-Content $log }
    }
}
