# Releasing a new ClipShelf version

The version is decided **at build time** by the release script. It looks at
the latest released `vX.Y.Z` git tag, suggests the next version, and stamps
your choice into `src-tauri/tauri.conf.json` + `src-tauri/Cargo.toml`. You
never edit a version number by hand.

## The one command

```powershell
.\build-release.ps1                          # interactive: suggests next version
.\build-release.ps1 -Version 2.1.0           # non-interactive, exact version
.\build-release.ps1 -Bump minor              # bump from the latest released tag
.\build-release.ps1 -Version 2.1.0 -Notes "..."   # with user-facing release notes
.\build-release.ps1 -LocalOnly               # build + pack only, no git/GitHub
```

The script:

1. Refuses to run if the working tree has uncommitted changes, if `gh` isn't
   logged in, or if the chosen tag already exists.
2. Suggests the next version from the latest released tag (if the manifest
   was already pre-bumped ahead of the tags, it suggests exactly that).
3. Stamps the version into `tauri.conf.json` + `Cargo.toml`.
4. Builds with `pnpm tauri build`, downloads the previous release from
   GitHub (`vpk download github`) so Velopack can generate a **delta**
   package, then packs with `vpk pack` into `src-tauri\Releases\vX.Y.Z\`
   (git-ignored) and renames the setup to `ClipShelf_X.Y.Z_x64-setup.exe`
   plus copies the portable exe.
5. Commits `Release vX.Y.Z`, creates an annotated tag, pushes branch + tag.
6. Publishes a GitHub release on `Bristopher/ClipShelf` with the Velopack
   feed files (`releases.win.json`, `RELEASES`, `*.nupkg`) **under their
   original names** — the in-app updater fetches them through
   `releases/latest/download/` — plus the setup and portable exes for
   manual installs.

## How users get it

On next launch (or Settings → *Check for updates now*, or tray →
*Check for updates*) the app compares the GitHub feed against its own
version and **asks** the user; nothing updates silently.

- **Installed builds** (Velopack setup): accepting downloads the
  delta/full package and restarts into the new version.
- **Portable/dev builds**: Velopack can't self-update those, so accepting
  opens https://github.com/Bristopher/ClipShelf/releases/latest for a
  manual download.

The startup check can be disabled in Settings → Updates
(`check_updates` in config).

## Requirements

- `pnpm` (frontend deps installed)
- `vpk` — Velopack CLI (`dotnet tool install -g vpk`)
- `gh` CLI logged in to the Bristopher account (`gh auth status`)

## If something goes wrong mid-release

The script is safe to re-run after you fix the cause, but clean up whatever
half-happened first:

```powershell
git tag -d vX.Y.Z                      # if the tag was created
git push origin :refs/tags/vX.Y.Z      # if it was pushed
gh release delete vX.Y.Z               # if the release was published
git reset --hard HEAD~1                # if the release commit was made
```
