# build-release.ps1 — run from project root: .\build-release.ps1
$ErrorActionPreference = "Stop"
$ProjectRoot = $PSScriptRoot

# ── Detect next version ──────────────────────────────────────────────────────
# Seed the suggestion from the highest of: (a) the existing Releases folders and
# (b) the version already baked into tauri.conf.json. Taking the max of both
# guards against ever suggesting a version BEHIND the shipped manifest (which
# would write a downgrade into the app's Settings display) if the Releases
# folder is cleaned or the manifest was hand-bumped ahead of it.
$ReleasesDir = Join-Path $ProjectRoot "src-tauri\Releases"
$tauriConf   = Join-Path $ProjectRoot "src-tauri\tauri.conf.json"

# Collect every version we know about as sortable [version] objects.
$known = @()

if (Test-Path $ReleasesDir) {
    $known += Get-ChildItem $ReleasesDir -Directory |
        Where-Object { $_.Name -match '^v(\d+)\.(\d+)\.(\d+)$' } |
        ForEach-Object { [version]($_.Name.TrimStart('v')) }
}

# The version currently in tauri.conf.json (the one the app shows in Settings).
$confRaw = Get-Content $tauriConf -Raw
if ($confRaw -match '"version":\s*"(\d+\.\d+\.\d+)"') {
    $known += [version]$Matches[1]
    Write-Host "Current manifest version: v$($Matches[1])"
}

if ($known.Count -gt 0) {
    $latest = $known | Sort-Object | Select-Object -Last 1
    $suggestedVersion = "$($latest.Major).$($latest.Minor).$($latest.Build + 1)"
    Write-Host "Highest known version: v$latest"
} else {
    $suggestedVersion = "2.0.0"
}

# ── Prompt ────────────────────────────────────────────────────────────────────
Write-Host "Suggested next version: v$suggestedVersion"
$userInput = Read-Host "Press Enter to accept, or type a custom version (e.g. 2.1.0)"
$version = if ($userInput.Trim() -ne "") { $userInput.Trim().TrimStart('v') } else { $suggestedVersion }

Write-Host ""
Write-Host "Building v$version..." -ForegroundColor Cyan

# ── Update tauri.conf.json (this is what the app shows in Settings) ───────────
$conf = Get-Content $tauriConf -Raw
$conf = $conf -replace '"version":\s*"\d+\.\d+\.\d+"', "`"version`": `"$version`""
Set-Content $tauriConf $conf -Encoding UTF8 -NoNewline
Write-Host "  Updated tauri.conf.json -> $version"

# ── Update Cargo.toml ─────────────────────────────────────────────────────────
$cargoToml = Join-Path $ProjectRoot "src-tauri\Cargo.toml"
$cargo = Get-Content $cargoToml -Raw
$cargo = $cargo -replace '(?m)^version = "\d+\.\d+\.\d+"', "version = `"$version`""
Set-Content $cargoToml $cargo -Encoding UTF8 -NoNewline
Write-Host "  Updated Cargo.toml -> $version"

# ── Step 1: Tauri build ───────────────────────────────────────────────────────
Write-Host ""
Write-Host "Step 1 — Building with Tauri..." -ForegroundColor Yellow
Set-Location $ProjectRoot

# Try cargo tauri first, fall back to pnpm tauri
$tauriCmd = $null
if (Get-Command "cargo-tauri" -ErrorAction SilentlyContinue) {
    $tauriCmd = "cargo"
    $tauriArgs = @("tauri", "build")
} elseif (Get-Command "pnpm" -ErrorAction SilentlyContinue) {
    $tauriCmd = "pnpm"
    $tauriArgs = @("tauri", "build")
} else {
    throw "Neither 'cargo tauri' nor 'pnpm' found. Install tauri-cli: cargo install tauri-cli --version '^2'"
}

& $tauriCmd @tauriArgs
if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }

# ── Step 2: Velopack pack ─────────────────────────────────────────────────────
Write-Host ""
Write-Host "Step 2 — Packaging with Velopack..." -ForegroundColor Yellow

$outDir = Join-Path $ProjectRoot "src-tauri\Releases\v$version"
if (Test-Path $outDir) {
    Write-Host ""
    Write-Host "  WARNING: Release v$version already exists at:" -ForegroundColor Yellow
    Write-Host "  $outDir" -ForegroundColor White
    $overwrite = Read-Host "  Overwrite? [y/N]"
    if ($overwrite.Trim().ToLower() -ne "y") {
        throw "Aborted — release v$version already exists."
    }
    Remove-Item $outDir -Recurse -Force
    Write-Host "  Deleted existing release folder." -ForegroundColor DarkGray
}

Set-Location (Join-Path $ProjectRoot "src-tauri")
vpk pack --packId com.cbuzi.gkey-mover-v2 --packTitle "GKey Mover" --packVersion $version --packDir "target/release" --mainExe "gkey-mover-v2.exe" --icon "icons/icon.ico" --outputDir "Releases/v$version"
if ($LASTEXITCODE -ne 0) { throw "vpk pack failed" }

# ── Step 3: Rename setup installer ────────────────────────────────────────────
Write-Host ""
Write-Host "Step 3 — Renaming release files..." -ForegroundColor Yellow
$outDir = Join-Path $ProjectRoot "src-tauri\Releases\v$version"

$setupSrc = Join-Path $outDir "com.cbuzi.gkey-mover-v2-win-Setup.exe"
$setupDst = Join-Path $outDir "GKeyMover_${version}_x64-setup.exe"
if (Test-Path $setupSrc) {
    Rename-Item $setupSrc $setupDst
    Write-Host "  Setup   -> GKeyMover_${version}_x64-setup.exe" -ForegroundColor Green
}

# ── Step 4: Copy portable exe ────────────────────────────────────────────────
Write-Host ""
Write-Host "Step 4 — Copying portable exe..." -ForegroundColor Yellow
$portableSrc = Join-Path $ProjectRoot "src-tauri\target\release\gkey-mover-v2.exe"
$portableDst = Join-Path $outDir "GKeyMover_${version}_x64-Portable.exe"
if (Test-Path $portableSrc) {
    Copy-Item $portableSrc $portableDst
    Write-Host "  Portable -> GKeyMover_${version}_x64-Portable.exe" -ForegroundColor Green
}

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "Done! Release v$version ready at:" -ForegroundColor Green
Get-ChildItem $outDir | ForEach-Object {
    $sizeMB = [math]::Round($_.Length / 1MB, 1)
    Write-Host "  $($_.Name) (${sizeMB} MB)" -ForegroundColor White
}
Set-Location $ProjectRoot
