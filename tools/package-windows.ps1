param([switch]$PortableOnly,[switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    if (-not $SkipBuild) {
    & cargo build --locked --release -p code-musume-hook
    if ($LASTEXITCODE -ne 0) { throw 'CLI release build failed' }
    $buildArgs = @('--filter', '@code-musume/pet', 'tauri', 'build', '--config', 'src-tauri/tauri.release.conf.json')
    if ($PortableOnly) { $buildArgs += '--no-bundle' }
    & pnpm @buildArgs
    if ($LASTEXITCODE -ne 0) { throw 'Tauri release build failed' }
    }
    $version = (Get-Content -LiteralPath 'apps/pet/src-tauri/tauri.conf.json' -Encoding UTF8 -Raw | ConvertFrom-Json).version
    $revision = (& git rev-parse --short HEAD)
    if ($LASTEXITCODE -ne 0 -or -not $revision) { throw 'Cannot read the source revision' }
    # One directory per version and commit, so older packages are never picked up or hashed.
    $output = Join-Path $repo ('artifacts/' + $version + '-' + $revision.Trim())
    if (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output -Recurse -Force }
    $stage = Join-Path $output ('code-musume-' + $version + '-windows-x64')
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    Copy-Item -LiteralPath 'target/release/pet.exe' -Destination (Join-Path $stage 'code-musume.exe') -Force
    Copy-Item -LiteralPath 'target/release/code-musume-hook.exe' -Destination $stage -Force
    Copy-Item -LiteralPath 'docs/使用说明-2026-10-14.md' -Destination (Join-Path $stage 'README.md') -Force
    Copy-Item -LiteralPath 'docs/使用说明-2026-10-14.md' -Destination (Join-Path $stage '使用说明.md') -Force
    if (Test-Path -LiteralPath 'LICENSE') { Copy-Item -LiteralPath 'LICENSE' -Destination $stage -Force }
    Compress-Archive -LiteralPath $stage -DestinationPath ($stage + '.zip') -Force
    $setup = $null
    if (-not $PortableOnly) {
        # The bundle directory can still hold installers from earlier versions.
        $built = Join-Path $repo ('target/release/bundle/nsis/code-musume_' + $version + '_x64-setup.exe')
        if (-not (Test-Path -LiteralPath $built)) { throw ('Installer not found: ' + $built) }
        $setup = Join-Path $output (Split-Path -Leaf $built)
        Copy-Item -LiteralPath $built -Destination $setup -Force
    }
    $hashes = @(Get-ChildItem -LiteralPath $output -File | Where-Object { $_.Extension -in '.exe','.zip' } | ForEach-Object {
        $stream = [IO.File]::OpenRead($_.FullName)
        $sha = [Security.Cryptography.SHA256]::Create()
        try { [pscustomobject]@{file=$_.Name;sha256=([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant()} }
        finally { $stream.Dispose();$sha.Dispose() }
    })
    $hashes | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'checksums.json') -Encoding UTF8
    Write-Output ('Packages: ' + $output)
    if ($setup) { Write-Output ('SETUP=' + $setup) }
} finally { Pop-Location }
