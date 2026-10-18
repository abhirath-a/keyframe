param(
    [Parameter(Mandatory = $true)][string]$Binary,
    [string]$Document,
    [string]$Label = 'keyframe',
    [int]$WarmupSeconds = 4
)

# Windows-only benchmark. Measure a fresh process without trimming its working
# set. Private working set excludes shared DLL pages; private commit also
# includes allocations that are not currently resident in physical RAM.
$start = @{
    FilePath = (Resolve-Path -LiteralPath $Binary).Path
    WindowStyle = 'Hidden'
    PassThru = $true
}
if ($Document) {
    $start.ArgumentList = '"' + (Resolve-Path -LiteralPath $Document).Path + '"'
}
$appProcess = Start-Process @start
try {
    Start-Sleep -Seconds $WarmupSeconds
    $measurements = @()
    foreach ($sample in 1..3) {
        $appProcess.Refresh()
        if ($appProcess.HasExited) { throw 'App exited during benchmark' }
        $memory = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess = $($appProcess.Id)"
        $measurements += [pscustomobject]@{
            label = $Label
            sample = $sample
            pid = $appProcess.Id
            workingSetMiB = [math]::Round($appProcess.WorkingSet64 / 1MB, 2)
            privateWorkingSetMiB = [math]::Round($memory.WorkingSetPrivate / 1MB, 2)
            privateCommittedMiB = [math]::Round($appProcess.PrivateMemorySize64 / 1MB, 2)
        }
        Start-Sleep -Seconds 2
    }
    $measurements | ConvertTo-Json
} finally {
    $appProcess.Refresh()
    if (!$appProcess.HasExited) { $appProcess.CloseMainWindow() | Out-Null }
    $appProcess.Dispose()
}
