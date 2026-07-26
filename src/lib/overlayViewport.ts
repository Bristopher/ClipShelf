/**
 * Rolodex viewport helper — computes scroll offset and ellipsis visibility
 * for a fixed-height list (e.g., 7-row visible window over N rows).
 * Role: MicGuard mixer_viewport port
 *
 * @param nRows total number of rows in the list
 * @param selected index of the selected row (0-based)
 * @param offset current scroll offset (top row index, 0-based)
 * @param visible number of visible rows in the viewport (default 7)
 * @returns { offset, dotsAbove, dotsBelow } — new offset to scroll to,
 *          and whether ellipsis should show above/below the viewport
 */
/**
 * Compact "time since clip" caption for the overlay's thumbnail strip.
 * <1m → "now", <60m → "12m ago", otherwise "1h 05m". Null (unparseable
 * timestamp) falls back to the provided clock string.
 */
export function formatClipAge(ageMin: number | null, fallback: string): string {
  if (ageMin === null || ageMin < 0) return fallback;
  if (ageMin < 1) return "now";
  if (ageMin < 60) return `${ageMin}m ago`;
  const h = Math.floor(ageMin / 60);
  const m = ageMin % 60;
  return `${h}h ${String(m).padStart(2, "0")}m`;
}

export function overlayViewport(nRows: number, selected: number, offset: number, visible = 7) {
  if (nRows <= visible) return { offset: 0, dotsAbove: false, dotsBelow: false };
  let off = Math.max(0, Math.min(offset, nRows - visible));
  if (selected < off) off = selected;
  else if (selected >= off + visible) off = selected - visible + 1;
  return { offset: off, dotsAbove: off > 0, dotsBelow: off + visible < nRows };
}
