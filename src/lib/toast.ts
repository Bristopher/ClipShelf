// Tiny module-level toast store — no library, no React context. Any code
// (including non-React code like the main.tsx unhandledrejection handler)
// can call toast()/toastError(); the <Toaster /> mounted in each window
// subscribes and renders whatever is queued.

export type ToastKind = "error" | "info" | "success";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

let nextId = 1;
let toasts: ToastItem[] = [];
const listeners = new Set<(toasts: ToastItem[]) => void>();
const timers = new Map<number, number>();

function notify() {
  const snapshot = [...toasts];
  listeners.forEach((l) => l(snapshot));
}

export function toast(kind: ToastKind, message: string, durationMs = 6000) {
  // Collapse exact duplicates (e.g. mashing a failing hotkey) — refresh the
  // existing toast's timer instead of stacking identical messages.
  const existing = toasts.find((t) => t.kind === kind && t.message === message);
  if (existing) {
    const timer = timers.get(existing.id);
    if (timer) window.clearTimeout(timer);
    timers.set(
      existing.id,
      window.setTimeout(() => dismissToast(existing.id), durationMs),
    );
    return;
  }

  const item: ToastItem = { id: nextId++, kind, message };
  toasts = [...toasts, item].slice(-4); // never more than 4 on screen
  timers.set(
    item.id,
    window.setTimeout(() => dismissToast(item.id), durationMs),
  );
  notify();
}

export const toastError = (message: string) => toast("error", message);
export const toastInfo = (message: string) => toast("info", message);
export const toastSuccess = (message: string) => toast("success", message);

export function dismissToast(id: number) {
  const timer = timers.get(id);
  if (timer) window.clearTimeout(timer);
  timers.delete(id);
  toasts = toasts.filter((t) => t.id !== id);
  notify();
}

export function subscribeToasts(listener: (toasts: ToastItem[]) => void) {
  listeners.add(listener);
  listener([...toasts]); // deliver anything queued before mount
  return () => {
    listeners.delete(listener);
  };
}

/** Normalize a rejected invoke() reason (string | Error | unknown) for display. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
