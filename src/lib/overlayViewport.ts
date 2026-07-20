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
export function overlayViewport(nRows: number, selected: number, offset: number, visible = 7) {
  if (nRows <= visible) return { offset: 0, dotsAbove: false, dotsBelow: false };
  let off = Math.max(0, Math.min(offset, nRows - visible));
  if (selected < off) off = selected;
  else if (selected >= off + visible) off = selected - visible + 1;
  return { offset: off, dotsAbove: off > 0, dotsBelow: off + visible < nRows };
}
