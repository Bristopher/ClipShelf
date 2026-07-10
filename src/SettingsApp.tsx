import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "@/components/ui/button";
import { getConfig, updateConfig } from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { refreshSystemMode } from "@/lib/systemTheme";
import { useTheme } from "@/hooks/useTheme";
import { SettingsForm } from "@/components/SettingsForm";
import { WindowChrome } from "@/components/WindowChrome";
import { Toaster } from "@/components/Toaster";
import { errorMessage, toastError, toastSuccess } from "@/lib/toast";
import type { AppConfig } from "@/types";

const appWindow = getCurrentWindow();

/**
 * The settings window is now a form: edits go to a `draft` copy, persisted
 * only on Save. Cancel reverts to the last saved snapshot. The window can't
 * be closed while dirty — attempts trigger a scroll-to-button-bar + shake
 * so the user has to consciously Save or "Exit Without Saving".
 *
 * ThemePanel is the one component that auto-persists (theme management has
 * heavy state — custom themes, import/export). When ThemePanel writes via
 * `updateConfig`, we sync `saved` AND `draft` so the user's other in-flight
 * edits aren't clobbered.
 */
export function SettingsApp() {
  const [saved, setSaved] = useState<AppConfig | null>(null);
  const [draft, setDraft] = useState<AppConfig | null>(null);
  const [shaking, setShaking] = useState(false);
  const buttonBarRef = useRef<HTMLDivElement>(null);
  const scrollAreaRef = useRef<HTMLDivElement>(null);

  useTheme(draft, null);

  useEffect(() => {
    getConfig()
      .then((c) => {
        setSaved(c);
        setDraft(c);
      })
      .catch(console.error);
    refreshSystemMode().catch(() => {});
  }, []);

  // Other windows (or theme operations) can mutate config — keep in sync.
  // Only update `saved`; preserve user's pending non-conflicting drafts.
  useEffect(() => {
    const unlisten = listen<AppConfig>(EVENTS.CONFIG_CHANGED, (event) => {
      setSaved(event.payload);
      // If the user has no pending edits, also resync the draft.
      setDraft((current) => {
        if (!current || !saved) return event.payload;
        const same = JSON.stringify(current) === JSON.stringify(saved);
        return same ? event.payload : current;
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [saved]);

  const dirty = useMemo(() => {
    if (!saved || !draft) return false;
    return JSON.stringify(saved) !== JSON.stringify(draft);
  }, [saved, draft]);

  const handleSave = async () => {
    if (!draft) return;
    try {
      // The backend appends to rename_mru on every rename — never send the
      // draft's (possibly stale) copy or a save here would clobber entries
      // added while this window sat open. updateConfig merges partially, so
      // omitting the field keeps the backend value.
      const { rename_mru: _mru, ...payload } = draft;
      const updated = await updateConfig(payload);
      setSaved(updated);
      setDraft(updated);
      toastSuccess("Settings saved");
    } catch (err) {
      console.error(err);
      toastError(`Save failed: ${errorMessage(err)}`);
    }
  };

  const handleExitWithoutSaving = () => {
    if (saved) setDraft(saved);
    appWindow.hide();
  };

  // Scroll the button bar into view and shake it. Used both when the user
  // clicks the X with unsaved changes and when Tauri fires a close-request
  // event (Alt+F4 etc.).
  const flashButtonBar = () => {
    buttonBarRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
    setShaking(false);
    // Force re-trigger of the animation if it's already running.
    requestAnimationFrame(() => {
      setShaking(true);
      window.setTimeout(() => setShaking(false), 600);
    });
  };

  const onCloseRequest = (): boolean => {
    if (!dirty) return true;
    flashButtonBar();
    return false;
  };

  // Ctrl+S saves — the whole window is a save-gated form, so give it the
  // universal form shortcut. Ref so the one listener sees fresh state.
  const saveRef = useRef({ dirty, handleSave });
  saveRef.current = { dirty, handleSave };
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (saveRef.current.dirty) saveRef.current.handleSave();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // Catch Alt+F4 / right-click-titlebar-close at the OS level.
  useEffect(() => {
    let unlisten: undefined | (() => void);
    appWindow
      .onCloseRequested((event) => {
        if (dirty) {
          event.preventDefault();
          flashButtonBar();
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [dirty]);

  if (!draft) {
    return (
      <div className="flex flex-col h-screen bg-app-bg text-t-text">
        <WindowChrome title="Settings" />
        <div className="flex-1 flex items-center justify-center">
          <p className="text-t-muted">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen bg-app-bg text-t-text">
      <WindowChrome title="Settings" onCloseRequest={onCloseRequest} />
      <div ref={scrollAreaRef} className="flex-1 overflow-y-auto">
        <div className="max-w-xl mx-auto px-6 py-6 pb-24">
          <SettingsForm
            config={draft}
            onConfigChange={(c) => {
              setDraft(c);
              // ThemePanel writes immediately via updateConfig and forwards
              // the persisted result here. Keep `saved` in sync so the
              // dirty check accounts for it.
              setSaved((prev) => {
                if (!prev) return c;
                // Heuristic: if only theme-related fields differ from prev
                // saved, treat this as a persisted change.
                const themeKeys: (keyof AppConfig)[] = [
                  "active_theme_id",
                  "themes",
                ];
                const onlyThemeChanged = (
                  Object.keys(c) as (keyof AppConfig)[]
                ).every((k) =>
                  themeKeys.includes(k) || JSON.stringify(c[k]) === JSON.stringify(prev[k]),
                );
                return onlyThemeChanged
                  ? { ...prev, active_theme_id: c.active_theme_id, themes: c.themes }
                  : prev;
              });
            }}
          />
          <div
            ref={buttonBarRef}
            className={`mt-6 sticky bottom-0 -mx-6 px-6 py-3 border-t border-t-border bg-app-bg/95 backdrop-blur flex items-center justify-end gap-2 ${
              shaking ? "settings-shake" : ""
            }`}
          >
            {dirty && (
              <span className="text-[10px] text-t-muted mr-auto">
                Unsaved changes
              </span>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={handleExitWithoutSaving}
              className="h-8 text-xs"
            >
              {dirty ? "Exit without saving" : "Close"}
            </Button>
            <Button
              size="sm"
              onClick={handleSave}
              disabled={!dirty}
              className="h-8 text-xs"
              title="Save (Ctrl+S)"
            >
              Save
            </Button>
          </div>
        </div>
      </div>
      <Toaster />
    </div>
  );
}
