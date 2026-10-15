param(
    [Parameter(Mandatory=$true)][string]$NativeRuntime,
    [Parameter(Mandatory=$true)][string]$CaptureDir,
    [int]$Seconds=240,
    [ValidateSet('auto','dev','release')][string]$Mode='auto',
    [string]$Versions='{}'
)
$ErrorActionPreference='Stop'
& node (Join-Path $PSScriptRoot 'capture-live-hooks.mjs') --runtime $NativeRuntime --dir $CaptureDir --seconds $Seconds --mode $Mode --versions $Versions
if($LASTEXITCODE -ne 0){throw 'Hook capture failed; inspect the capture report.'}
