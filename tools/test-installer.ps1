param([string]$Setup='artifacts/code-musume_0.1.0_x64-setup.exe')
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$case=Join-Path $repo ('.test-artifacts/install-'+[guid]::NewGuid().ToString('N'))
$destination=Join-Path $case 'app'
$project=Join-Path $case 'project'
New-Item -ItemType Directory -Path $project -Force|Out-Null
$priorData=$env:CODE_MUSUME_DATA_DIR;$priorConfig=$env:CODE_MUSUME_CONFIG_DIR
$env:CODE_MUSUME_DATA_DIR=Join-Path $case 'runtime';$env:CODE_MUSUME_CONFIG_DIR=Join-Path $case 'config'
$checks=[Collections.Generic.List[string]]::new()
function Assert-That($value,[string]$label){if(-not $value){throw ('FAILED: '+$label)};$checks.Add($label)}
try{
    # The test must not remove an existing user's autostart choice.
    $runKey=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Run')
    $existing=if($runKey){try{$runKey.GetValue('CodeMusume')}finally{$runKey.Dispose()}}else{$null}
    if($existing){throw 'Existing CodeMusume autostart found: run this test under a clean Windows account.'}
    $installer=Start-Process -FilePath ([IO.Path]::GetFullPath((Join-Path $repo $Setup))) -ArgumentList @('/S',('/D='+$destination)) -WindowStyle Hidden -PassThru
    Assert-That ($installer.WaitForExit(60000)) 'installer exits'
    Assert-That ($installer.ExitCode -eq 0) 'installer success'
    $cli=Join-Path $destination 'code-musume-hook.exe'
    Assert-That (Test-Path -LiteralPath $cli) 'hook CLI included in installation'
    Assert-That (Test-Path -LiteralPath (Join-Path $destination 'pet.exe')) 'pet executable installed'
    Assert-That (@(Get-ChildItem -LiteralPath $destination -Filter '*.md').Count -ge 1) 'Chinese user guide included'
    foreach($agent in 'claude-code','codex'){
        $preview=& $cli install --agent $agent --cwd $project --runtime release
        if($LASTEXITCODE -ne 0){throw 'Preview failed'}
        $digest=($preview|ConvertFrom-Json).digest
        & $cli install --agent $agent --cwd $project --runtime release --confirm --expected-digest $digest|Out-Null
        if($LASTEXITCODE -ne 0){throw 'Test hook installation failed'}
    }
    $claudeFile=Join-Path $project '.claude/settings.json'
    $value=Get-Content -LiteralPath $claudeFile -Encoding UTF8 -Raw|ConvertFrom-Json
    $value|Add-Member -NotePropertyName userSetting -NotePropertyValue 42
    [IO.File]::WriteAllText($claudeFile,($value|ConvertTo-Json -Depth 20),[Text.UTF8Encoding]::new($false))
    $uninstaller=Start-Process -FilePath (Join-Path $destination 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru
    Assert-That ($uninstaller.WaitForExit(60000)) 'uninstaller launcher exits'
    $deadline=[DateTime]::UtcNow.AddSeconds(30)
    while((Test-Path -LiteralPath $cli) -and [DateTime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 250}
    Assert-That (-not (Test-Path -LiteralPath $cli)) 'application files removed'
    $after=Get-Content -LiteralPath $claudeFile -Encoding UTF8 -Raw|ConvertFrom-Json
    Assert-That ($after.userSetting -eq 42 -and $null -eq $after.hooks) 'external config edit preserved and own Claude hooks removed'
    Assert-That (-not (Test-Path -LiteralPath (Join-Path $project '.codex/hooks.json'))) 'untouched Codex config restored to absence'
    Assert-That (@(Get-ChildItem -LiteralPath (Join-Path $env:CODE_MUSUME_DATA_DIR 'installs') -Filter '*.json').Count -eq 0) 'install receipts cleaned'
    @{at=[DateTime]::UtcNow.ToString('o');checks=$checks;note='Current Windows account, isolated install directory and hook receipts; not a clean-user acceptance test.'}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $case 'result.json') -Encoding UTF8
    Write-Output ('PASS '+$checks.Count+' checks; '+$case)
}finally{$env:CODE_MUSUME_DATA_DIR=$priorData;$env:CODE_MUSUME_CONFIG_DIR=$priorConfig}
