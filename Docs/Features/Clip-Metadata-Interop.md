# Clip Metadata Interop — Reading Gkey Mover Data From Other Apps

**Status:** Partially implemented (Phase 1, shipped 2026-07-12,
spec: `Docs/specs/2026-07-12-game-detection-history-overlay-design.md`).
`history.jsonl` (§3) is live for `created`/`moved`/`renamed`/`undone` events with
game context, and the game name is written to `System.Keywords` (§2). Everything
else below is still design-only: `rated`/`labeled`/`described`/`game_edited`
history events, the `System.Rating`/`System.Comment` property writes, and the
filename label suffix convention (§1) — those land with the overlay in Phase 3.
Field names and mappings below are the contract — implementation must match this doc, and
changes to the scheme must update this doc in the same change.

Gkey Mover records clip info in three places. Any external app can read all three with no
dependency on Gkey Mover itself.

| Where | What lives there | Survives file move/rename? |
|---|---|---|
| Filename | label suffix, `{date}`/`{time}` rename tokens | n/a (it IS the name) |
| Windows file properties (in the MP4) | game (Tags), rating (stars), description (Comment) | yes — travels with the file |
| `history.jsonl` | every event with full context | yes — but paths go stale if files are moved by other tools |

---

## 1. Filename conventions

- **Label suffix:** appended before the extension as ` - <label>`:
  `2026-07-12 21.34.56.mp4` → `2026-07-12 21.34.56 - clutch.mp4`
  Parse rule: everything after the last ` - ` (space-hyphen-space) before the extension is
  the label. Collision suffixes use the app's existing ` (n)` pattern.
- **Rename tokens** (already shipped): `{date}` expands to `YYYY-MM-DD`, `{time}` to
  `HH.MM` (dots, not colons — colons are illegal in Windows filenames).

## 2. Windows file properties

Written via the Windows Shell property store (no re-encode; stored inside the MP4's
metadata atoms, so they travel with the file and show in Explorer's Details pane).

| Data | Property key | Format |
|---|---|---|
| Game name | `System.Keywords` (Explorer "Tags") | the game name as one tag, e.g. `Counter-Strike 2` or `Desktop-Discord` |
| Star rating | `System.Rating` | Explorer's 1–99 scale: 1★=1, 2★=25, 3★=50, 4★=75, 5★=99 |
| Description | `System.Comment` | free text |

Star conversion: `stars = {1:1, 25:2, 50:3, 75:4, 99:5}[value]` (tolerant read:
1–12→1★, 13–37→2★, 38–62→3★, 63–87→4★, 88–99→5★ matches Explorer's buckets).

### PowerShell

```powershell
$shell  = New-Object -ComObject Shell.Application
$folder = $shell.Namespace('C:\clips')
$item   = $folder.ParseName('clip - clutch.mp4')

$folder.GetDetailsOf($item, 18)   # Tags (game), e.g. "Counter-Strike 2"
$folder.GetDetailsOf($item, 19)   # Rating, e.g. "4 Stars"
$folder.GetDetailsOf($item, 24)   # Comments (description)
```

Column indexes can vary by Windows build — resolve them once by name instead:

```powershell
0..320 | ForEach-Object {
  $n = $folder.GetDetailsOf($null, $_)
  if ($n -in 'Tags','Rating','Comments') { "{0} = {1}" -f $_, $n }
}
```

### Python (pywin32)

```python
from win32com.propsys import propsys, pscon

ps = propsys.SHGetPropertyStoreFromParsingName(r"C:\clips\clip - clutch.mp4")
game        = ps.GetValue(pscon.PKEY_Keywords).GetValue()   # ['Counter-Strike 2']
rating_raw  = ps.GetValue(pscon.PKEY_Rating).GetValue()     # 75
description = ps.GetValue(pscon.PKEY_Comment).GetValue()

stars = next(s for lo, s in [(88,5),(63,4),(38,3),(13,2),(1,1),(0,0)] if (rating_raw or 0) >= lo)
```

### C# (.NET)

```csharp
// NuGet: Microsoft-WindowsAPICodePack-Shell
using Microsoft.WindowsAPICodePack.Shell;

var f = ShellFile.FromFilePath(@"C:\clips\clip - clutch.mp4");
string[] game   = f.Properties.System.Keywords.Value;   // ["Counter-Strike 2"]
uint?   rating  = f.Properties.System.Rating.Value;     // 75 → 4 stars
string  comment = f.Properties.System.Comment.Value;
```

### Node.js

No native property-store binding worth taking a dependency on — shell out:

```js
const { execFileSync } = require("node:child_process");
const out = execFileSync("powershell", ["-NoProfile", "-Command", `
  $s=New-Object -ComObject Shell.Application; $f=$s.Namespace('C:\\clips');
  $i=$f.ParseName('clip - clutch.mp4');
  @{tags=$f.GetDetailsOf($i,18); rating=$f.GetDetailsOf($i,19); comment=$f.GetDetailsOf($i,24)} | ConvertTo-Json
`]).toString();
const props = JSON.parse(out);
```

`exiftool` also reads these (they live in the MP4 `Xtra` atom):
`exiftool -Keywords -SharedUserRating -Comment clip.mp4`.

## 3. `history.jsonl` — the full event log

Location: next to the app config. The config's primary home is the per-user
app-config dir resolved by Tauri — on Windows that is
`%APPDATA%\com.cbuzi.gkey-mover-v2\` (i.e.
`C:\Users\<you>\AppData\Roaming\com.cbuzi.gkey-mover-v2\`), so `history.jsonl`
sits there beside `gkey_config.toml` and `gkey_stats.toml`. A legacy
exe-adjacent location (`<install folder>\` next to the GKey Mover executable) is
still read as a fallback: if a config is found there and none exists in
`%APPDATA%` yet, it is migrated forward, and if Tauri cannot resolve the
app-config dir the app falls back to the exe-adjacent path.

Append-only JSON Lines, one event per line, kept forever. Fields absent when not
applicable; **skip unparseable lines** when reading (the app does too).

```json
{
  "ts": "2026-07-12T21:34:56-06:00",
  "event": "created | moved | renamed | rated | labeled | described | game_edited | undone",
  "path": "C:/clips/clip.mp4",
  "old_path": "C:/clips/old.mp4",
  "game": "Counter-Strike 2",
  "exe": "cs2",
  "key": 1,
  "rating": 4,
  "label": "clutch",
  "description": "1v4 on Mirage",
  "source": "hotkey | overlay | drop | app"
}
```

Notes for consumers:
- `ts` is RFC 3339 with local offset. The app's "day" starts at the configured rollover
  hour (default 4 AM) — bucket accordingly if you want to match its History panel.
- `exe` is optional and present on `created` events when detection ran; it records the
  detected process's exe stem (e.g. `cs2`) separately from the resolved `game` label —
  additive since 2026-07-12.
- `rating` here is **1–5 stars** (human scale); only the Windows property uses 1–99.
- The latest `moved`/`renamed` event for a clip has its current `path`; follow the
  `old_path` → `path` chain to track a file across events.
- Read-share the file (it may be appended to while the app runs); never write to it.

## 4. Gotchas

- **Don't write properties while OBS holds the file.** Gkey Mover probes for exclusive
  access before writing and retries; do the same if your app writes anything.
- Properties are **best-effort mirrors** — if a write was skipped (locked file),
  `history.jsonl` still has the data. Treat JSONL as the source of truth on disagreement.
- Filename label parsing breaks if a user manually renames with ` - ` in the middle;
  prefer the property/JSONL data when both exist.
