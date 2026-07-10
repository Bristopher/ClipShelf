# Future Feature Ideas (parking lot)

The 2026-07-05 parking lot shipped across the two 2026-07-10 batches (see
`Docs/specs/2026-07-10-feature-batch-design.md`): drag-and-drop sort (incl.
multi-file batch), per-G-key daily stats + flyout, rename MRU + {date}/{time}
tokens, log filter + context menu, richer empty state, first-run OBS section
with live test-connection, in-app shortcuts, diagnostics popover. What
remains below was deliberately deferred.

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

## Batch-drop undo-as-one (S-M)
Multi-file drops sort everything, but undo still reverses one file per
press. A batch-aware undo entry (Vec of from/to pairs) would restore a whole
drop at once — needs UX for partial-failure restores.
