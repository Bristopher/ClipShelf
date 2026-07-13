# Future Feature Ideas (parking lot)

The 2026-07-05 parking lot shipped across the two 2026-07-10 batches (see
`Docs/specs/2026-07-10-feature-batch-design.md`): drag-and-drop sort (incl.
multi-file batch), per-G-key daily stats + flyout, rename MRU + {date}/{time}
tokens, log filter + context menu, richer empty state, first-run OBS section
with live test-connection, in-app shortcuts, diagnostics popover,
batch-undo-as-one. What remains below was deliberately deferred.

## Rename `{n}` counter token (S-M)
`{date}`/`{time}` shipped. `{n}` needs counter semantics decided first: per
session? per day? per stem? Once decided, extend
`mover::expand_rename_tokens` (and the JS preview mirror in RenameDialog).

## Flyout thumbnails (M)
The per-G-key recent-clips flyout is text-only. Thumbnails need ffmpeg or
the Windows Shell thumbnail cache (IThumbnailProvider) — weigh the
dependency; shell cache is the lighter option.

## Weekly / all-time stats (S)
Daily counts persist in `gkey_stats.toml` and roll over at midnight. If
"this week" / "all time" ever matters, extend that file (it's already
load/save round-tripped and tested) rather than adding a new store.

The 2026-07-12 in-game overlay (Phase 3, spec:
`Docs/specs/2026-07-12-game-detection-history-overlay-design.md`) shipped a
deliberately narrower needs-label path than originally sketched, plus some
hardening left for later. Deferred:

## Needs-label queue (S-M)
Today, flagging a clip as "needs a label" from the overlay just writes a
Warning log entry — there's no persisted queue and no prompt when you're
back at the main app. If that reminder proves too easy to miss in practice,
persist flagged clips (path + timestamp, maybe in `gkey_stats.toml` or a
small sibling file) and surface a prompt/badge in the main window after the
game session ends.

## Overlay hardening (M)
A handful of robustness gaps accepted for the initial ship:
- The `WH_KEYBOARD_LL` hook currently does its work directly on the hook
  callback; decouple the emit path via a channel + forwarder thread so the
  hook procedure itself stays minimal and can't stall the OS input queue.
- Add a watchdog that auto-disarms type mode (unhooks and releases keys) if
  the frontend process dies while typing is active, instead of relying on
  the normal close path.
- Type mode currently passes Shift through but not Ctrl/Alt/Win — pass
  those through too so OS-level combos (e.g. Ctrl+Alt+Del intercepts aside)
  behave consistently with Shift.
- Seed the hook's shift state from `GetKeyState` when type mode starts,
  instead of only tracking transitions after entry — avoids the "Shift
  already held when type mode opens" cosmetic quirk noted in
  `Docs/Verify/2026-07-10-master-verification-checklist.md` §17.

## Overlay recent-games quick-pick (S)
The overlay's Game submenu currently offers keep/edit/remember rather than
a list of recently-used games. A recent-games quick-pick (last N distinct
games from `history.jsonl` or the override list) would make correcting
detection faster for anyone who bounces between a small rotation of games.

