param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$case=Join-Path $repo ('.test-artifacts/smoke-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $case -Force | Out-Null
$priorData=$env:CODE_MUSUME_DATA_DIR
$priorConfig=$env:CODE_MUSUME_CONFIG_DIR
$priorSeconds=$env:CODE_MUSUME_HEADLESS_SECONDS
$env:CODE_MUSUME_DATA_DIR=Join-Path $case 'runtime'
$env:CODE_MUSUME_CONFIG_DIR=Join-Path $case 'config'
$env:CODE_MUSUME_HEADLESS_SECONDS='120'
$exe=Join-Path $repo ('target/'+$Profile+'/pet.exe')
$cli=Join-Path $repo ('target/'+$Profile+'/code-musume-hook.exe')
$mode=if($Profile -eq 'debug'){'dev'}else{'release'}
$runtime=Join-Path $env:CODE_MUSUME_DATA_DIR $(if($mode -eq 'dev'){'runtime.dev.json'}else{'runtime.json'})
$checks=[System.Collections.Generic.List[string]]::new()
function Assert-That($value,[string]$label){if(-not $value){throw ('FAILED: '+$label)};$checks.Add($label)}
function Send-Hook([string]$agent,[string]$event,[hashtable]$payload){
    $info=New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName=$cli
    $info.Arguments='--agent '+$agent+' --event '+$event+' --runtime '+$mode
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardInput=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $process=New-Object System.Diagnostics.Process
    $process.StartInfo=$info;[void]$process.Start()
    $bytes=[Text.Encoding]::UTF8.GetBytes(($payload|ConvertTo-Json -Depth 20 -Compress))
    $process.StandardInput.BaseStream.Write($bytes,0,$bytes.Length);$process.StandardInput.Close()
    Assert-That ($process.WaitForExit(1500)) 'hook deadline'
    Assert-That ($process.ExitCode -eq 0 -and $process.StandardOutput.ReadToEnd() -eq '' -and $process.StandardError.ReadToEnd() -eq '') 'silent hook'
    $process.Dispose()
}
try {
    if(-not (Test-Path -LiteralPath $exe)){throw 'Build pet and CLI first'}
    $app=Start-Process -FilePath $exe -ArgumentList '--headless' -WindowStyle Hidden -PassThru
    $deadline=[DateTime]::UtcNow.AddSeconds(15)
    while(-not (Test-Path -LiteralPath $runtime)){if($app.HasExited -or [DateTime]::UtcNow -gt $deadline){throw 'Core did not start'};Start-Sleep -Milliseconds 100}
    $r=Get-Content -LiteralPath $runtime -Raw|ConvertFrom-Json
    $url='http://127.0.0.1:'+$r.port
    $headers=@{Authorization='Bearer '+$r.token}
    function Snapshot { (Invoke-RestMethod -Uri ($url+'/v2/health') -Headers $headers -TimeoutSec 2).projection }
    function Check-Rejection([hashtable]$requestHeaders,[int]$expected){try{Invoke-WebRequest -UseBasicParsing -Uri ($url+'/v2/health') -Headers $requestHeaders -TimeoutSec 2|Out-Null;throw 'Unexpected success'}catch{if(-not $_.Exception.Response){throw};Assert-That ([int]$_.Exception.Response.StatusCode -eq $expected) ('HTTP '+$expected)}}
    Assert-That ((Snapshot).state -eq 'idle') 'initial idle without a window'
    Check-Rejection @{} 401
    Check-Rejection @{Authorization=$headers.Authorization;Origin='https://example.invalid'} 403
    $p=@{session_id='same';prompt_id='turn';turn_id='turn';tool_use_id='tool';tool_name='Bash';tool_input=@{command='fixture'}}
    $watch=[Diagnostics.Stopwatch]::StartNew()
    Send-Hook 'claude-code' 'PreToolUse' $p
    Assert-That ((Snapshot).state -eq 'working') 'Claude working'
    Assert-That ($watch.ElapsedMilliseconds -lt 1000) 'CLI-to-core under 1 second'
    $latency=$watch.ElapsedMilliseconds
    Send-Hook 'codex' 'PreToolUse' $p
    Assert-That ((Snapshot).activeSessions -eq 2) 'same ID is isolated by source'
    Send-Hook 'claude-code' 'PermissionRequest' $p
    Send-Hook 'codex' 'Stop' $p
    Assert-That ((Snapshot).pendingCount -eq 1) 'other source stop preserves waiting'
    Send-Hook 'claude-code' 'PostToolUse' $p
    Assert-That ((Snapshot).pendingCount -eq 0) 'matched tool resolves waiting'
    Send-Hook 'claude-code' 'Stop' $p
    Assert-That ((Snapshot).state -eq 'done') 'round done'
    Start-Sleep -Milliseconds 3300
    Assert-That ((Snapshot).state -eq 'idle') 'native timer expires done without frontend'
    $bad=@{v=1;id='bad';agent='codex';event='Stop';ts=0;src=@{cli='fixture';pid=1};payload='{}'}|ConvertTo-Json -Compress
    try{Invoke-RestMethod -Method Post -Uri ($url+'/v2/event') -Headers $headers -ContentType 'application/json' -Body $bad|Out-Null;throw 'v1 unexpectedly accepted'}catch{if(-not $_.Exception.Response){throw};Assert-That ([int]$_.Exception.Response.StatusCode -eq 400) 'v1 rejected'}
    $result=@{at=[DateTime]::UtcNow.ToString('o');profile=$Profile;synthetic=$true;checks=$checks;latencyMs=$latency;note='Does not validate real agents, trust or rendered latency.'}
    $result|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $case 'result.json') -Encoding UTF8
    Write-Output ('PASS '+$checks.Count+' checks; '+$case)
} finally {
    if($app -and -not $app.HasExited){Stop-Process -Id $app.Id -ErrorAction SilentlyContinue}
    $env:CODE_MUSUME_DATA_DIR=$priorData;$env:CODE_MUSUME_CONFIG_DIR=$priorConfig;$env:CODE_MUSUME_HEADLESS_SECONDS=$priorSeconds
}
