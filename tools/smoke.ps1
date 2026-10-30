param([ValidateSet('debug','release')][string]$Profile='debug',[string]$BinDir='')
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
$binaryRoot=if($BinDir){[IO.Path]::GetFullPath($BinDir)}else{Join-Path $repo ('target/'+$Profile)}
$exe=Join-Path $binaryRoot 'pet.exe'
$cli=Join-Path $binaryRoot 'code-musume-hook.exe'
$mode=if($Profile -eq 'debug'){'dev'}else{'release'}
$runtime=Join-Path $env:CODE_MUSUME_DATA_DIR $(if($mode -eq 'dev'){'runtime.dev.json'}else{'runtime.json'})
$checks=[System.Collections.Generic.List[string]]::new()
$script:hookSequence=0
function Assert-That($value,[string]$label){if(-not $value){throw ('FAILED: '+$label)};$checks.Add($label)}
function Send-Hook([string]$agent,[string]$event,[hashtable]$payload){
    $payload.hook_event_name=$event
    # Preparing a fixture must not consume the hook's 300ms process budget.
    # ConvertTo-Json can take longer on its first Windows PowerShell invocation.
    $bytes=[Text.Encoding]::UTF8.GetBytes(($payload|ConvertTo-Json -Depth 20 -Compress))
    $info=New-Object System.Diagnostics.ProcessStartInfo
    $script:hookSequence++
    $diagnosticFile=Join-Path $case ('hook-{0:D3}.json' -f $script:hookSequence)
    $info.FileName=$cli
    $info.Arguments='--agent '+$agent+' --event '+$event+' --runtime '+$mode+' --diagnostic-file "'+$diagnosticFile+'"'
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardInput=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $process=New-Object System.Diagnostics.Process
    $process.StartInfo=$info
    # .NET Framework creates a StreamWriter using Console.InputEncoding and may
    # immediately emit its BOM. Raw BaseStream writes alone do not prevent that.
    $priorInputEncoding=[Console]::InputEncoding
    try {
        [Console]::InputEncoding=New-Object Text.UTF8Encoding $false
        [void]$process.Start()
    } finally { [Console]::InputEncoding=$priorInputEncoding }
    $writerPreambleBytes=$process.StandardInput.Encoding.GetPreamble().Length
    $process.StandardInput.BaseStream.Write($bytes,0,$bytes.Length);$process.StandardInput.Close()
    Assert-That ($process.WaitForExit(1500)) 'hook deadline'
    Assert-That ($process.ExitCode -eq 0 -and $process.StandardOutput.ReadToEnd() -eq '' -and $process.StandardError.ReadToEnd() -eq '') 'silent hook'
    try {
        $trace=Get-Content -LiteralPath $diagnosticFile -Raw -Encoding UTF8|ConvertFrom-Json
        $sender=@{agent=$agent;event=$event;traceId=$trace.traceId;outcome=$trace.outcome;httpStatus=$trace.httpStatus;input=$trace.input;elapsedMs=$trace.elapsedMs;consoleEncoding=$priorInputEncoding.WebName;consolePreambleBytes=$priorInputEncoding.GetPreamble().Length;writerPreambleBytes=$writerPreambleBytes}
        Write-Output ('TRACE hook '+($sender|ConvertTo-Json -Depth 5 -Compress))
    } catch { Write-Output ('TRACE hook report-unavailable; sequence='+$script:hookSequence+'; errorType='+$_.Exception.GetType().Name) }
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
    function Save-DiagnosticFailure {
        try {
            $detail=Invoke-RestMethod -Uri ($url+'/v2/diagnostics') -Headers $headers -TimeoutSec 2
            $detail|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $case 'failure-diagnostics.json') -Encoding UTF8
            $invalid=$detail.pipeline.entries|Where-Object {$_.input -and -not $_.input.jsonValid}|Select-Object -Last 1
            $core=$detail.pipeline.entries|Where-Object {$_.stage -eq 'core'}|Select-Object -Last 1
            Write-Output ('TRACE failure '+(@{state=$detail.projection.state;pending=$detail.projection.pendingCount;missed=$detail.projection.missedEvents;inputError=$invalid.input.errorKind;inputBom=$invalid.input.utf8Bom;coreOutcome=$core.outcome;eventId=$core.eventId;revision=$detail.projection.revision;queue=$detail.queue}|ConvertTo-Json -Depth 5 -Compress))
        } catch { Write-Output 'TRACE failure diagnostics-unavailable' }
    }
    function Snapshot { (Invoke-RestMethod -Uri ($url+'/v2/health') -Headers $headers -TimeoutSec 2).projection }
    function Await-Snapshot([scriptblock]$condition,[string]$label){
        $deadline=[DateTime]::UtcNow.AddMilliseconds(1000)
        do {
            $snapshot=Snapshot
            if((& $condition $snapshot)){Assert-That $true $label;return $snapshot}
            Start-Sleep -Milliseconds 20
        } while([DateTime]::UtcNow -lt $deadline)
        $snapshot|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $case 'failure-projection.json') -Encoding UTF8
        throw ('FAILED: '+$label+'; state='+$snapshot.state+'; pending='+$snapshot.pendingCount+'; missed='+$snapshot.missedEvents+'; evidence='+$case)
    }
    function Check-Rejection([hashtable]$requestHeaders,[int]$expected){try{Invoke-WebRequest -UseBasicParsing -Uri ($url+'/v2/health') -Headers $requestHeaders -TimeoutSec 2|Out-Null;throw 'Unexpected success'}catch{if(-not $_.Exception.Response){throw};Assert-That ([int]$_.Exception.Response.StatusCode -eq $expected) ('HTTP '+$expected)}}
    Assert-That ((Snapshot).state -eq 'idle') 'initial idle without a window'
    Check-Rejection @{} 401
    Check-Rejection @{Authorization=$headers.Authorization;Origin='https://example.invalid'} 403
    function Tool-Fixture([string]$agent){
        $scenario=Get-Content -LiteralPath (Join-Path $repo ('fixtures/scenarios/'+$agent+'-normal.json')) -Encoding UTF8 -Raw | ConvertFrom-Json
        $source=($scenario.steps | Where-Object {$_.action.type -eq 'event' -and $_.action.event -eq 'PreToolUse'} | Select-Object -First 1).action.payload
        $payload=@{};foreach($property in $source.PSObject.Properties){$payload[$property.Name]=$property.Value};return $payload
    }
    $p=Tool-Fixture 'claude-code'
    $cx=Tool-Fixture 'codex'
    $watch=[Diagnostics.Stopwatch]::StartNew()
    Send-Hook 'claude-code' 'PreToolUse' $p
    Await-Snapshot {param($p) $p.state -eq 'working'} 'Claude working' | Out-Null
    Assert-That ($watch.ElapsedMilliseconds -lt 1000) 'CLI-to-core under 1 second'
    $latency=$watch.ElapsedMilliseconds
    Send-Hook 'codex' 'PreToolUse' $cx
    Assert-That ((Snapshot).activeSessions -eq 2) 'same ID is isolated by source'
    Send-Hook 'claude-code' 'PermissionRequest' $p
    Send-Hook 'codex' 'Stop' $cx
    Assert-That ((Snapshot).pendingCount -eq 1) 'other source stop preserves waiting'
    Send-Hook 'claude-code' 'PostToolUse' $p
    Assert-That ((Snapshot).pendingCount -eq 0) 'matched tool resolves waiting'
    Send-Hook 'claude-code' 'Stop' $p
    Assert-That ((Snapshot).state -eq 'done') 'round done'
    Start-Sleep -Milliseconds 3300
    Assert-That ((Snapshot).state -eq 'idle') 'native timer expires done without frontend'
    $bad=@{v=1;id='bad';agent='codex';event='Stop';ts=0;src=@{cli='fixture';pid=1};payload='{}'}|ConvertTo-Json -Compress
    try{Invoke-RestMethod -Method Post -Uri ($url+'/v2/event') -Headers $headers -ContentType 'application/json' -Body $bad|Out-Null;throw 'v1 unexpectedly accepted'}catch{if(-not $_.Exception.Response){throw};Assert-That ([int]$_.Exception.Response.StatusCode -eq 400) 'v1 rejected'}
    # Deliberately invalid input must be explainable without saving its contents.
    $probe=@{v=2;id='diagnostic-bom-probe';agent='claude-code';event='PreToolUse';ts=0;src=@{cli='fixture';pid=1};payload=([string][char]0xfeff+'{"session_id":"private-diagnostic-session","prompt":"never-save-diagnostic-content"}');parseError=@{kind='json'}}|ConvertTo-Json -Depth 5 -Compress
    Invoke-RestMethod -Method Post -Uri ($url+'/v2/event') -Headers $headers -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($probe))|Out-Null
    $detail=Invoke-RestMethod -Uri ($url+'/v2/diagnostics') -Headers $headers -TimeoutSec 2
    $inputTrace=$detail.pipeline.entries|Where-Object {$_.input.utf8Bom}|Select-Object -Last 1
    Assert-That ($inputTrace.input.errorKind -eq 'utf8-bom') 'diagnostics identifies input BOM'
    $coreTrace=$detail.pipeline.entries|Where-Object {$_.eventId -eq $inputTrace.eventId -and $_.stage -eq 'core'}|Select-Object -Last 1
    Assert-That ($coreTrace.outcome -eq 'invalid-payload') 'diagnostics correlates HTTP input and core rejection'
    $safeReport=$detail|ConvertTo-Json -Depth 20
    Assert-That (-not $safeReport.Contains('never-save-diagnostic-content') -and -not $safeReport.Contains('private-diagnostic-session') -and -not $safeReport.Contains($r.token)) 'diagnostics omits payload and token'
    $safeReport|Set-Content -LiteralPath (Join-Path $case 'pipeline-verification.json') -Encoding UTF8
    $result=@{at=[DateTime]::UtcNow.ToString('o');profile=$Profile;synthetic=$true;checks=$checks;latencyMs=$latency;sourceCommit=$env:GITHUB_SHA;powerShellVersion=$PSVersionTable.PSVersion.ToString();note='Does not validate real agents, trust or rendered latency.'}
    $result|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $case 'result.json') -Encoding UTF8
    Write-Output ('PASS '+$checks.Count+' checks; '+$case)
} catch {
    if($url -and $headers){Save-DiagnosticFailure}
    throw
} finally {
    if($app -and -not $app.HasExited){Stop-Process -Id $app.Id -ErrorAction SilentlyContinue}
    $env:CODE_MUSUME_DATA_DIR=$priorData;$env:CODE_MUSUME_CONFIG_DIR=$priorConfig;$env:CODE_MUSUME_HEADLESS_SECONDS=$priorSeconds
}
