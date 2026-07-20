import { useRef, useState } from "react";

/**
 * Themed hover tooltip — replaces native `title=` (the unstyled OS yellow
 * box) everywhere. Same look as the title bar's tooltips: popover colors,
 * rounded corners, arrow, ~350ms delay, fade/zoom in. Supports multi-line
 * via `\n` in `text` and an optional dimmer `sub` line.
 */
export function Tip({
  text,
  sub,
  side = "top",
  align = "center",
  delay = 350,
  wrapperClass = "",
  children,
}: {
  text: string;
  sub?: string;
  /** Which side of the target the bubble appears on. */
  side?: "top" | "bottom";
  /** Horizontal anchoring of the bubble relative to the target. */
  align?: "center" | "left" | "right";
  delay?: number;
  /** Extra classes for the wrapping span (e.g. flex sizing). */
  wrapperClass?: string;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const timer = useRef<number | null>(null);
  const show = () => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setOpen(true), delay);
  };
  const hide = () => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = null;
    setOpen(false);
  };

  const sideClass = side === "top" ? "bottom-full mb-1.5" : "top-full mt-1.5";
  const alignClass =
    align === "center" ? "left-1/2 -translate-x-1/2" : align === "left" ? "left-0" : "right-0";
  const arrowX =
    align === "center" ? "left-1/2 -translate-x-1/2" : align === "left" ? "left-3" : "right-3";
  const arrowY =
    side === "top"
      ? "-bottom-1 border-r border-b"
      : "-top-1 border-l border-t";

  return (
    <span
      className={`relative inline-flex ${wrapperClass}`}
      onMouseEnter={show}
      onMouseLeave={hide}
      onMouseDown={hide}
    >
      {children}
      {open && (
        <span
          className={`absolute ${sideClass} ${alignClass} pointer-events-none z-50`}
          aria-hidden="true"
        >
          <span className="block relative px-2.5 py-1 rounded-md bg-popover text-popover-foreground text-[11px] font-medium shadow-lg border border-border whitespace-pre-line text-left max-w-[280px] w-max animate-in fade-in-0 zoom-in-95 duration-150">
            {text}
            {sub && (
              <span className="block text-[9px] text-muted-foreground mt-0.5 truncate max-w-[260px]">
                {sub}
              </span>
            )}
            <span className={`absolute w-2 h-2 bg-popover rotate-45 border-border ${arrowX} ${arrowY}`} />
          </span>
        </span>
      )}
    </span>
  );
}
