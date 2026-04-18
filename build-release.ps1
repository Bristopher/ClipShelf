# build-release.ps1 — run from project root: .\build-release.ps1
$ErrorActionPreference = "Stop"
$ProjectRoot = $PSScriptRoot

# ── Detect next version from existing Releases folders ───────────────────────
$ReleasesDir = Join-Path $ProjectRoot "src-tauri\Releases"
$suggestedVersion = "2.0.0"

if (Test-Path $ReleasesDir) {
    $latest = Get-ChildItem $ReleasesDir -Directory |
        Where-Object { $_.Name -match '^v(\d+)\.(\d+)\.(\d+)$' } |
        ForEach-Object {
            $null = $_.Name -match '^v(\d+)\.(\d+)\.(\d+)$'
            [PSCustomObject]@{ Major = [int]$Matches[1]; Minor = [int]$Matches[2]; Patch = [int]$Matches[3] }
        } |
        Sort-Object Major, Minor, Patch |
        Select-Object -Last 1

    if ($latest) {
        $suggestedVersion = "$($latest.Major).$($latest.Minor).$($latest.Patch + 1)"
        Write-Host "Latest release: v$($latest.Major).$($latest.Minor).$($latest.Patch)"
    }
}

# ── Prompt ────────────────────────────────────────────────────────────────────
Write-Host "Suggested next version: v$suggestedVersion"
$userInput = Read-Host "Press Enter to accept, or type a custom version (e.g. 2.1.0)"
$version = if ($userInput.Trim() -ne "") { $userInput.Trim().TrimStart('v') } else { $suggestedVersion }

Write-Host ""
Write-Host "Building v$version..." -ForegroundColor Cyan

# ── Update tauri.conf.json ────────────────────────────────────────────────────
$tauriConf = Join-Path $ProjectRoot "src-tauri\tauri.conf.json"
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
vpk pack --packId com.cbuzi.gkey-mover-v2 --packTitle "Gkey Mover v2" --packVersion $version --packDir "target/release" --mainExe "gkey-mover-v2.exe" --icon "icons/icon.ico" --outputDir "Releases/v$version"
if ($LASTEXITCODE -ne 0) { throw "vpk pack failed" }

# ── Step 3: Rename setup installer ────────────────────────────────────────────
Write-Host ""
Write-Host "Step 3 — Renaming release files..." -ForegroundColor Yellow
$outDir = Join-Path $ProjectRoot "src-tauri\Releases\v$version"

$setupSrc = Join-Path $outDir "com.cbuzi.gkey-mover-v2-win-Setup.exe"
$setupDst = Join-Path $outDir "GkeyMoverV2_${version}_x64-setup.exe"
if (Test-Path $setupSrc) {
    Rename-Item $setupSrc $setupDst
    Write-Host "  Setup   -> GkeyMoverV2_${version}_x64-setup.exe" -ForegroundColor Green
}

# ── Step 4: Copy portable exe ────────────────────────────────────────────────
Write-Host ""
Write-Host "Step 4 — Copying portable exe..." -ForegroundColor Yellow
$portableSrc = Join-Path $ProjectRoot "src-tauri\target\release\gkey-mover-v2.exe"
$portableDst = Join-Path $outDir "GkeyMoverV2_${version}_x64-Portable.exe"
if (Test-Path $portableSrc) {
    Copy-Item $portableSrc $portableDst
    Write-Host "  Portable -> GkeyMoverV2_${version}_x64-Portable.exe" -ForegroundColor Green
}

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "Done! Release v$version ready at:" -ForegroundColor Green
Get-ChildItem $outDir | ForEach-Object {
    $sizeMB = [math]::Round($_.Length / 1MB, 1)
    Write-Host "  $($_.Name) (${sizeMB} MB)" -ForegroundColor White
}
Set-Location $ProjectRoot
