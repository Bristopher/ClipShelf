# Future Feature Ideas (parking lot)

Most of the 2026-07-05 parking lot shipped in the 2026-07-10 feature batch
(see `Docs/specs/2026-07-10-feature-batch-design.md`): drag-and-drop sort,
per-G-key stats + flyout, rename MRU, log filter + context menu, richer
empty state, first-run OBS section, in-app shortcuts, diagnostics popover.
What remains below are the pieces deliberately deferred out of those.

## Rename token templates (M)
The MRU chips shipped; the power version is templates with tokens like
`{game} {date} {n}`. `{n}` needs counter semantics (per session? per day?
per stem?) — decide that before building. Needs: token expansion (frontend
or in `mover::rename_file_with_text`), template management UI.

## Flyout thumbnails (M)
The per-G-key recent-clips flyout is text-only. Thumbnails need ffmpeg or
the Windows Shell thumbnail cache (IThumbnailProvider) — weigh the
dependency; shell cache is the lighter option.

## Persistent per-G-key stats (S)
Stats are session-only (reset on launch). A "today" counter that survives
restarts needs a small persisted file with a date-rollover — decide whether
daily/weekly/all-time before adding.

## First-run OBS live test-connection (S-M)
The first-run OBS section saves credentials and points at the status dot
for feedback. A one-shot "Test" button needs a single-attempt connect
(current `connect_and_run` is loop-oriented). Revisit if users get confused
by the save-then-look flow.

## Multi-file drag-drop batch sort (M)
Dropping multiple videos currently uses the first and toasts about the
rest. Batch mode (sort ALL dropped files to the target key) is easy on the
backend (loop the shared move path) but needs UX for partial failures and
undo-of-a-batch semantics.
