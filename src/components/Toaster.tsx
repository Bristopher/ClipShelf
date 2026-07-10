import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { EVENTS } from "@/lib/events";
import {
  dismissToast,
  subscribeToasts,
  toastError,
  type ToastItem,
} from "@/lib/toast";

interface ToasterProps {
  /**
   * Also surface backend `error` events (watcher/OBS/hotkey/move failures).
   * Enable only in the main window — backend events broadcast to every
   * window, and one visible toast per error is enough.
   */
  listenBackendErrors?: boolean;
}

const kindStyle: Record<ToastItem["kind"], string> = {
  error: "border-red-500/50 text-red-300",
  info: "border-t-border text-t-text",
  success: "border-green-500/50 text-green-300",
};

function KindIcon({ kind }: { kind: ToastItem["kind"] }) {
  const cls = "h-3.5 w-3.5 shrink-0 mt-[1px]";
  if (kind === "error") return <AlertCircle className={cls} />;
  if (kind === "success") return <CheckCircle2 className={cls} />;
  return <Info className={cls} />;
}

export function Toaster({ listenBackendErrors = false }: ToasterProps) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  useEffect(() => subscribeToasts(setToasts), []);

  useEffect(() => {
    if (!listenBackendErrors) return;
    const unlisten = listen<{ message: string; context: string }>(
      EVENTS.ERROR,
      (e) => {
        toastError(e.payload.message);
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [listenBackendErrors]);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-9 right-2 z-[100] flex flex-col gap-1.5 items-end pointer-events-none">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`pointer-events-auto flex items-start gap-1.5 max-w-[340px] px-2.5 py-1.5 rounded-md bg-popover border shadow-lg text-[11px] leading-snug animate-in fade-in-0 slide-in-from-bottom-2 duration-150 ${kindStyle[t.kind]}`}
        >
          <KindIcon kind={t.kind} />
          <span className="break-words min-w-0">{t.message}</span>
          <button
            onClick={() => dismissToast(t.id)}
            className="shrink-0 opacity-60 hover:opacity-100 mt-[1px]"
            aria-label="Dismiss"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      ))}
    </div>
  );
}
