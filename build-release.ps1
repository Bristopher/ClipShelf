# build-release.ps1 — one command releases a new GKey Mover version.
# See RELEASING.md. Requires: pnpm, vpk (Velopack CLI), gh (logged in).
#
# Usage:
#   .\build-release.ps1                    # interactive: suggests next version
#   .\build-release.ps1 -Version 2.1.0     # non-interactive, exact version
#   .\build-release.ps1 -Bump minor        # bump from the latest released tag
#   .\build-release.ps1 -Version 2.1.0 -Notes "..."   # with release notes
#   .\build-release.ps1 -LocalOnly         # build + pack only (no git/GitHub)
param(
    [string]$Version = '',
    [ValidateSet('patch', 'minor', 'major')]
    [string]$Bump = '',
    [string]$Notes = '',
    [switch]$LocalOnly
)
$ErrorActionPreference = "Stop"
$ProjectRoot = $PSScriptRoot
Set-Location $ProjectRoot

$GithubRepo = "Bristopher/GKeyMover"

# ── Guards ────────────────────────────────────────────────────────────────────
if (-not $LocalOnly) {
    # The version-stamped files are exempt: a failed/aborted release leaves
    # them dirty, and the script itself commits exactly those.
    $dirty = git status --porcelain | Where-Object {
        $_ -notmatch 'src-tauri/(tauri\.conf\.json|Cargo\.(toml|lock))$'
    }
    if ($dirty) {
        throw "Working tree is not clean - commit or stash your changes first."
    }
    gh auth status *> $null
    if ($LASTEXITCODE -ne 0) { throw "gh CLI is not logged in - run: gh auth login" }
    git remote get-url origin *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "No 'origin' remote - create the GitHub repo first: gh repo create GKeyMover --public --source . --push"
    }
}

# ── Detect next version from released git tags (MicGuard-style) ──────────────
$tauriConf = Join-Path $ProjectRoot "src-tauri\tauri.conf.json"
$confRaw = Get-Content $tauriConf -Raw
if ($confRaw -notmatch '"version":\s*"(\d+)\.(\d+)\.(\d+)"') {
    throw 'version "x.y.z" not found in tauri.conf.json'
}
$current = "$($Matches[1]).$($Matches[2]).$($Matches[3])"

# "Released" = any vX.Y.Z git tag, PLUS any src-tauri\Releases\vX.Y.Z folder
# (pre-GitHub releases were only packaged locally — without this, the first
# tagged release suggests a version that's already shipped).
$released = @(
    git tag --list 'v*' | Where-Object { $_ -match '^v(\d+)\.(\d+)\.(\d+)$' }
)
$releasesDir = Join-Path $ProjectRoot "src-tauri\Releases"
if (Test-Path $releasesDir) {
    $released += Get-ChildItem $releasesDir -Directory |
        Where-Object { $_.Name -match '^v(\d+)\.(\d+)\.(\d+)$' } |
        ForEach-Object { $_.Name }
}
$latestReleased = $released |
    ForEach-Object { [version]($_.TrimStart('v')) } |
    Sort-Object | Select-Object -Last 1

if ($latestReleased) {
    Write-Host "Latest released:       v$latestReleased"
    # If the manifest is already ahead of every release (pre-bumped for a
    # local test build), suggest releasing exactly that; else bump the patch.
    $suggested = if ([version]$current -gt $latestReleased) { $current } else {
        "$($latestReleased.Major).$($latestReleased.Minor).$($latestReleased.Build + 1)"
    }
    $bumpBase = $latestReleased
} else {
    $suggested = $current
    $bumpBase = [version]$current
}
Write-Host "Manifest version:      v$current"

# ── Decide the version: -Version > -Bump > interactive prompt ────────────────
if ($Version) {
    $new = $Version.Trim().TrimStart('v')
} elseif ($Bump) {
    $major, $minor, $patch = $bumpBase.Major, $bumpBase.Minor, $bumpBase.Build
    switch ($Bump) {
        'major' { $major++; $minor = 0; $patch = 0 }
        'minor' { $minor++; $patch = 0 }
        'patch' { $patch++ }
    }
    $new = "$major.$minor.$patch"
} else {
    Write-Host "Suggested next version: v$suggested"
    $userInput = Read-Host "Press Enter to accept, or type a custom version (e.g. 2.1.0)"
    $new = if ($userInput.Trim() -ne "") { $userInput.Trim().TrimStart('v') } else { $suggested }
}
if ($new -notmatch '^\d+\.\d+\.\d+$') { throw "Invalid version '$new' - expected x.y.z" }
if (-not $LocalOnly -and (git tag --list "v$new")) {
    throw "Tag v$new already exists - pick a different version."
}
Write-Host ""
Write-Host "Releasing v$new" -ForegroundColor Green

# ── Stamp tauri.conf.json + Cargo.toml ────────────────────────────────────────
$conf = $confRaw -replace '"version":\s*"\d+\.\d+\.\d+"', "`"version`": `"$new`""
Set-Content $tauriConf $conf -Encoding UTF8 -NoNewline

$cargoToml = Join-Path $ProjectRoot "src-tauri\Cargo.toml"
(Get-Content $cargoToml -Raw) -replace '(?m)^version = "\d+\.\d+\.\d+"', "version = `"$new`"" |
    Set-Content $cargoToml -Encoding UTF8 -NoNewline
Write-Host "  Stamped tauri.conf.json + Cargo.toml -> $new"

# ── Step 1: Tauri build ───────────────────────────────────────────────────────
Write-Host ""
Write-Host "Step 1 - Building with Tauri..." -ForegroundColor Yellow
if (Get-Command "cargo-tauri" -ErrorAction SilentlyContinue) {
    cargo tauri build
} else {
    pnpm tauri build
}
if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }

# ── Step 2: Velopack pack (with deltas against the published feed) ───────────
Write-Host ""
Write-Host "Step 2 - Packaging with Velopack..." -ForegroundColor Yellow

$outDir = Join-Path $ProjectRoot "src-tauri\Releases\v$new"
if (Test-Path $outDir) {
    Write-Host "  Release v$new already exists at $outDir" -ForegroundColor Yellow
    $overwrite = Read-Host "  Overwrite? [y/N]"
    if ($overwrite.Trim().ToLower() -ne "y") { throw "Aborted." }
    Remove-Item $outDir -Recurse -Force
}

Set-Location (Join-Path $ProjectRoot "src-tauri")

# Download the previously published release from GitHub first so vpk can
# generate a DELTA package (small download for updaters). Only attempted
# when a release actually exists - a bare repo would just 404 noisily.
if (-not $LocalOnly) {
    gh release view --repo $GithubRepo *> $null
    if ($LASTEXITCODE -eq 0) {
        vpk download github --repoUrl "https://github.com/$GithubRepo" --outputDir "Releases/v$new"
        if ($LASTEXITCODE -ne 0) {
            Write-Host "  (delta download failed - building full package only)" -ForegroundColor DarkGray
        }
    } else {
        Write-Host "  (first GitHub release - skipping delta download)" -ForegroundColor DarkGray
    }
}

vpk pack --packId com.cbuzi.gkey-mover-v2 --packTitle "GKey Mover" --packVersion $new --packDir "target/release" --mainExe "gkey-mover-v2.exe" --icon "icons/icon.ico" --outputDir "Releases/v$new"
if ($LASTEXITCODE -ne 0) { throw "vpk pack failed" }

# ── Step 3: Rename setup + copy portable ──────────────────────────────────────
$setupSrc = Join-Path $outDir "com.cbuzi.gkey-mover-v2-win-Setup.exe"
$setupDst = Join-Path $outDir "GKeyMover_${new}_x64-setup.exe"
if (Test-Path $setupSrc) { Rename-Item $setupSrc $setupDst }

$portableSrc = Join-Path $ProjectRoot "src-tauri\target\release\gkey-mover-v2.exe"
$portableDst = Join-Path $outDir "GKeyMover_${new}_x64-Portable.exe"
if (Test-Path $portableSrc) { Copy-Item $portableSrc $portableDst }

Set-Location $ProjectRoot

if ($LocalOnly) {
    Write-Host ""
    Write-Host "Done (local only): v$new packaged at $outDir - nothing pushed or published." -ForegroundColor Green
    exit 0
}

# ── Step 4: Commit, tag, push ─────────────────────────────────────────────────
Write-Host ""
Write-Host "Step 4 - Commit, tag, push..." -ForegroundColor Yellow
git add "src-tauri/tauri.conf.json" "src-tauri/Cargo.toml" "src-tauri/Cargo.lock"
if (git status --porcelain) { git commit -m "Release v$new" }
git tag -a "v$new" -m "v$new"
git push origin main "v$new"

# ── Step 5: Publish the GitHub release ────────────────────────────────────────
# The Velopack feed files (releases.win.json, *.nupkg, RELEASES) MUST be
# attached with their original names - the in-app updater fetches them via
# releases/latest/download/. Setup + portable ride along for manual installs.
Write-Host ""
Write-Host "Step 5 - Publishing GitHub release..." -ForegroundColor Yellow
if (-not $Notes) { $Notes = "GKey Mover v$new" }

$assets = Get-ChildItem $outDir -File | Where-Object {
    $_.Name -in @("releases.win.json", "RELEASES", "assets.win.json") -or
    $_.Name -like "*.nupkg" -or
    $_.Name -like "GKeyMover_*"
} | ForEach-Object { $_.FullName }

gh release create "v$new" @assets --repo $GithubRepo --title "GKey Mover v$new" --notes $Notes
if ($LASTEXITCODE -ne 0) { throw "gh release create failed" }

Write-Host ""
Write-Host "Done: v$new published. Running apps will offer the update on next launch." -ForegroundColor Green
Get-ChildItem $outDir | ForEach-Object {
    $sizeMB = [math]::Round($_.Length / 1MB, 1)
    Write-Host "  $($_.Name) (${sizeMB} MB)"
}
