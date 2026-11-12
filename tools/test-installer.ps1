param([Parameter(Mandatory=$true)][string]$Setup)
# Pass the exact installer printed by package-windows.ps1 (SETUP=...). No search: picking
# "some" setup.exe is how an old package ends up being tested.
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$case=Join-Path $repo ('.test-artifacts/install-'+[guid]::NewGuid().ToString('N'))
$destination=Join-Path $case 'app'
$project=Join-Path $case 'project'
$tomlProject=Join-Path $case 'project-toml'
New-Item -ItemType Directory -Path $project -Force|Out-Null
New-Item -ItemType Directory -Path (Join-Path $tomlProject '.codex') -Force|Out-Null
$priorData=$env:CODE_MUSUME_DATA_DIR;$priorConfig=$env:CODE_MUSUME_CONFIG_DIR
$env:CODE_MUSUME_DATA_DIR=Join-Path $case 'runtime';$env:CODE_MUSUME_CONFIG_DIR=Join-Path $case 'config'
# The CLI prints UTF-8; Windows PowerShell would otherwise decode it with the console
# code page (e.g. GBK) and corrupt the Chinese warnings inside the JSON preview.
$priorOutputEncoding=[Console]::OutputEncoding
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$checks=[Collections.Generic.List[string]]::new()
$failure=$null
function Assert-That($value,[string]$label){if(-not $value){throw ('FAILED: '+$label)};$checks.Add($label)}
function Install-Hooks([string]$agent,[string]$cwd){
    $preview=& $cli install --agent $agent --cwd $cwd --runtime release
    if($LASTEXITCODE -ne 0){throw ('Preview failed: '+$agent+' '+$cwd)}
    $digest=(($preview -join "`n")|ConvertFrom-Json).digest
    & $cli install --agent $agent --cwd $cwd --runtime release --confirm --expected-digest $digest|Out-Null
    if($LASTEXITCODE -ne 0){throw ('Test hook installation failed: '+$agent+' '+$cwd)}
}
try{
    $setupPath=[IO.Path]::GetFullPath($Setup)
    if(-not (Test-Path -LiteralPath $setupPath -PathType Leaf)){throw ('Installer not found: '+$setupPath)}
    # The test must not remove an existing user's autostart choice.
    $runKey=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Run')
    $existing=if($runKey){try{$runKey.GetValue('CodeMusume')}finally{$runKey.Dispose()}}else{$null}
    if($existing){throw 'Existing CodeMusume autostart found: run this test under a clean Windows account.'}
    $installer=Start-Process -FilePath $setupPath -ArgumentList @('/S',('/D='+$destination)) -WindowStyle Hidden -PassThru
    Assert-That ($installer.WaitForExit(60000)) 'installer exits'
    Assert-That ($installer.ExitCode -eq 0) 'installer success'
    $cli=Join-Path $destination 'code-musume-hook.exe'
    Assert-That (Test-Path -LiteralPath $cli) 'hook CLI included in installation'
    Assert-That (Test-Path -LiteralPath (Join-Path $destination 'pet.exe')) 'pet executable installed'
    Assert-That (@(Get-ChildItem -LiteralPath $destination -Filter '*.md').Count -ge 1) 'Chinese user guide included'
    foreach($agent in 'claude-code','codex'){Install-Hooks $agent $project}
    # A Codex config.toml that already has inline hook arrays is edited in place.
    $tomlFile=Join-Path $tomlProject '.codex/config.toml'
    $tomlOriginal="model = `"fixture`"`n`n[[hooks.Stop]]`n[[hooks.Stop.hooks]]`ntype = `"command`"`ncommand = `"other-tool`"`n"
    [IO.File]::WriteAllText($tomlFile,$tomlOriginal,[Text.UTF8Encoding]::new($false))
    Install-Hooks 'codex' $tomlProject
    Assert-That (-not (Test-Path -LiteralPath (Join-Path $tomlProject '.codex/hooks.json'))) 'inline Codex TOML is the install target'
    Assert-That ((Get-Content -LiteralPath $tomlFile -Raw) -match 'EncodedCommand') 'own Codex hooks written to TOML'
    # External edits made after installation must survive uninstall.
    $claudeFile=Join-Path $project '.claude/settings.json'
    $value=Get-Content -LiteralPath $claudeFile -Encoding UTF8 -Raw|ConvertFrom-Json
    $value|Add-Member -NotePropertyName userSetting -NotePropertyValue 42
    [IO.File]::WriteAllText($claudeFile,($value|ConvertTo-Json -Depth 20),[Text.UTF8Encoding]::new($false))
    $tomlInstalled=[IO.File]::ReadAllText($tomlFile)
    [IO.File]::WriteAllText($tomlFile,("user_setting = 42`n"+$tomlInstalled),[Text.UTF8Encoding]::new($false))
    $uninstaller=Start-Process -FilePath (Join-Path $destination 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru
    Assert-That ($uninstaller.WaitForExit(60000)) 'uninstaller launcher exits'
    $deadline=[DateTime]::UtcNow.AddSeconds(30)
    while((Test-Path -LiteralPath $cli) -and [DateTime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 250}
    Assert-That (-not (Test-Path -LiteralPath $cli)) 'application files removed'
    $after=Get-Content -LiteralPath $claudeFile -Encoding UTF8 -Raw|ConvertFrom-Json
    Assert-That ($after.userSetting -eq 42 -and $null -eq $after.hooks) 'external config edit preserved and own Claude hooks removed'
    Assert-That (-not (Test-Path -LiteralPath (Join-Path $project '.codex/hooks.json'))) 'untouched Codex config restored to absence'
    $tomlAfter=[IO.File]::ReadAllText($tomlFile)
    Assert-That ($tomlAfter -match '(?m)^user_setting = 42' -and $tomlAfter -match 'other-tool' -and $tomlAfter -notmatch 'EncodedCommand') 'external TOML edit and other hooks preserved, own Codex hooks removed'
    Assert-That (@(Get-ChildItem -LiteralPath (Join-Path $env:CODE_MUSUME_DATA_DIR 'installs') -Filter '*.json').Count -eq 0) 'install receipts cleaned'
    Write-Output ('PASS '+$checks.Count+' checks; '+$case)
}catch{
    $failure=$_.Exception.Message
    if($failure.Length -gt 500){$failure=$failure.Substring(0,500)+'...'}
    throw
}finally{
    # A failed run must not leave the product installed (it registers an uninstall entry).
    $leftover=Join-Path $destination 'uninstall.exe'
    if(Test-Path -LiteralPath $leftover){
        $cleanup=Start-Process -FilePath $leftover -ArgumentList '/S' -WindowStyle Hidden -PassThru
        [void]$cleanup.WaitForExit(60000)
        $deadline=[DateTime]::UtcNow.AddSeconds(30)
        while((Test-Path -LiteralPath (Join-Path $destination 'pet.exe')) -and [DateTime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 250}
    }
    $env:CODE_MUSUME_DATA_DIR=$priorData;$env:CODE_MUSUME_CONFIG_DIR=$priorConfig
    [Console]::OutputEncoding=$priorOutputEncoding
    # Written on failure too, so CI always has evidence to upload.
    New-Item -ItemType Directory -Path $case -Force|Out-Null
    @{at=[DateTime]::UtcNow.ToString('o');setup=$Setup;passed=($null -eq $failure);failure=$failure;checks=$checks;note='Current Windows account, isolated install directory and hook receipts; not a clean-user acceptance test.'}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $case 'result.json') -Encoding UTF8
}
