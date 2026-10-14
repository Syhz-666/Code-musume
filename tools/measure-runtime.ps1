param([Parameter(Mandatory=$true)][string]$RuntimeFile,[int]$Seconds=600,[string]$Out='artifacts/runtime-measurement.json')
$ErrorActionPreference='Stop'
$r=Get-Content -LiteralPath $RuntimeFile -Raw|ConvertFrom-Json
$headers=@{Authorization='Bearer '+$r.token}
$url='http://127.0.0.1:'+$r.port+'/v2/health'
$samples=[Collections.Generic.List[object]]::new();$previous=@{};$last=[DateTime]::UtcNow
$end=$last.AddSeconds($Seconds)
while([DateTime]::UtcNow -lt $end){
    $all=@(Get-CimInstance Win32_Process|Select-Object ProcessId,ParentProcessId)
    $ids=[Collections.Generic.HashSet[int]]::new();[void]$ids.Add([int]$r.pid)
    do{$added=$false;foreach($p in $all){if($ids.Contains([int]$p.ParentProcessId) -and $ids.Add([int]$p.ProcessId)){$added=$true}}}while($added)
    $cpu=0.0;$memory=0L;$next=@{}
    $counters=@(Get-CimInstance Win32_PerfRawData_PerfProc_Process)
    foreach($id in $ids){$key=[string]$id;$stat=$counters|Where-Object {[int]$_.IDProcess -eq $id}|Select-Object -First 1;if($stat){$total=[double]$stat.PercentProcessorTime/10000000.0;$next[$key]=$total;if($previous.ContainsKey($key)){$cpu += [Math]::Max(0.0,[double]($total-$previous[$key]))};$memory += [long]$stat.WorkingSetPrivate}}
    $now=[DateTime]::UtcNow;$h=Invoke-RestMethod -Uri $url -Headers $headers -TimeoutSec 2
    if($previous.Count -gt 0){$samples.Add([pscustomobject]@{at=$now.ToString('o');state=$h.projection.state;cpuPercent=100*$cpu/($now-$last).TotalSeconds/[Environment]::ProcessorCount;privateWorkingSetMiB=$memory/1MB;renderFps=$h.renderFps;processCount=$ids.Count})}
    $previous=$next;$last=$now;Start-Sleep -Seconds 2
}
$idle=@($samples|Where-Object state -eq 'idle');$working=@($samples|Where-Object {($_.state -eq 'working' -or $_.state -eq 'thinking') -and $null -ne $_.renderFps})
$summary=@{durationSeconds=$Seconds;idleMeanCpu=($idle|Measure-Object cpuPercent -Average).Average;peakPrivateWorkingSetMiB=($samples|Measure-Object privateWorkingSetMiB -Maximum).Maximum;workingMeanFps=($working|Measure-Object renderFps -Average).Average;workingSamples=$working.Count;idleSamples=$idle.Count;samples=$samples;note='FPS is renderer-reported; null means no recent visible frontend report. Includes child WebView processes.'}
$absolute=[IO.Path]::GetFullPath($Out);New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($absolute)) -Force|Out-Null
$summary|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $absolute -Encoding UTF8
Write-Output $absolute
