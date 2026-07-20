import { useEffect, useRef, useState } from "react";
import { Settings } from "lucide-react";
import { emit, listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Tip } from "@/components/ui/tip";
import {
  getGkeyStats,
  openSettingsWindow,
  pressGkey,
  revealInExplorer,
} from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { errorMessage, toastError } from "@/lib/toast";
import type { AppConfig, GKeyStat } from "@/types";

interface SidebarProps {
  config: AppConfig;
  /** G-key button a dragged file is currently hovering (1-4), or null. */
  dropKey: number | null;
}

/** Compact button tag from the configured folder name (max 5 chars). */
function tagFor(folderName: string, fallback: string): string {
  const name = folderName.trim();
  if (!name) return fallback;
  return name.length <= 5 ? name.toUpperCase() : name.slice(0, 4).toUpperCase() + "…";
}

export function Sidebar({ config, dropKey }: SidebarProps) {
  const sorting = !config.disable_file_movesorting;
  // Session move stats per key — badge on the button, recent clips in the
  // hover flyout. Refetched whenever a move lands.
  const [stats, setStats] = useState<Record<number, GKeyStat>>({});
  const [flyoutKey, setFlyoutKey] = useState<number | null>(null);
  const flyoutTimer = useRef<number | null>(null);

  useEffect(() => {
    const fetchStats = () =>
      getGkeyStats()
        .then((list) => {
          const byKey: Record<number, GKeyStat> = {};
          for (const s of list) byKey[s.key] = s;
          setStats(byKey);
        })
        .catch(() => {});
    fetchStats();
    const unlisten = listen(EVENTS.FILE_MOVED, fetchStats);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const openFlyout = (key: number) => {
    if (flyoutTimer.current) window.clearTimeout(flyoutTimer.current);
    flyoutTimer.current = window.setTimeout(() => setFlyoutKey(key), 500);
  };
  const closeFlyout = () => {
    if (flyoutTimer.current) window.clearTimeout(flyoutTimer.current);
    setFlyoutKey(null);
  };

  // Tags reflect the user's actual configured folder names — hardcoded
  // labels went stale the moment a folder was renamed in Settings.
  const gkeys = [
    {
      key: 1,
      label: "G1",
      tag: tagFor(config.g1_bind_folder_name, "G1"),
      folder: config.g1_bind_folder_name,
      accent: "var(--t-g1-accent)",
    },
    {
      key: 2,
      label: "G2",
      tag: tagFor(config.g2_bind_folder_name, "G2"),
      folder: config.g2_bind_folder_name,
      accent: "var(--t-g2-accent)",
    },
    {
      key: 3,
      label: "G3",
      tag: tagFor(config.g3_bind_folder_name, "G3"),
      folder: config.g3_bind_folder_name,
      accent: "var(--t-g3-accent)",
    },
    {
      key: 4,
      label: "G4",
      tag: "REN",
      folder: "",
      accent: "var(--t-g4-accent)",
    },
  ];

  return (
    <aside className="w-16 border-r border-t-border flex flex-col gap-1 p-1.5 bg-panel">
      {gkeys.map((g) => {
        const stat = g.key <= 3 ? stats[g.key] : undefined;
        return (
          <div
            key={g.key}
            className="relative"
            // Counts persist across launches but the recent list is
            // session-only — no flyout until something moved this session.
            onMouseEnter={() => (stat?.recent.length ? openFlyout(g.key) : undefined)}
            onMouseLeave={closeFlyout}
          >
            <button
              // G4 opens the rename dialog (press_gkey rejects key 4 — it's
              // not a move bind). The dialog listens for hotkey-triggered
              // key=4.
              onClick={() =>
                g.key === 4
                  ? emit(EVENTS.HOTKEY_TRIGGERED, { key: 4 }).catch(console.error)
                  : pressGkey(g.key).catch((e) => toastError(errorMessage(e)))
              }
              style={{ backgroundColor: g.accent }}
              data-drop-key={g.key}
              title={
                g.key === 4
                  ? "Rename the current clip"
                  : sorting
                    ? `Move current clip to "${g.folder || g.label}"`
                    : `Tag current clip as "${g.folder || g.label}" (rename only)`
              }
              aria-label={g.key === 4 ? "Rename clip" : `Sort clip to ${g.folder || g.label}`}
              className={`w-full rounded px-1.5 py-1.5 text-white text-[10px] font-bold flex flex-col items-center gap-0 transition-[filter,transform] hover:brightness-110 active:scale-95 ${
                dropKey === g.key ? "ring-2 ring-white brightness-125" : ""
              }`}
            >
              <span>{g.label}</span>
              <span className="text-[8px] opacity-80 leading-tight max-w-full truncate pointer-events-none">
                {g.tag}
              </span>
              {stat && stat.count > 0 && (
                <span className="absolute -top-0.5 -right-0.5 min-w-3.5 px-0.5 h-3.5 rounded-full bg-black/60 text-white text-[8px] leading-3.5 text-center font-semibold pointer-events-none">
                  {stat.count}
                </span>
              )}
            </button>
            {flyoutKey === g.key && stat && stat.recent.length > 0 && (
              <div className="absolute left-full top-0 ml-1.5 z-50 w-56 rounded-md border border-t-border bg-panel shadow-lg p-2 animate-in fade-in-0 zoom-in-95 duration-150">
                <p className="text-[10px] font-semibold text-t-text truncate">
                  {g.folder || g.label} — {stat.count} today
                </p>
                <div className="mt-1 space-y-0.5">
                  {stat.recent.map((clip) => (
                    <Tip
                      key={clip.path}
                      text="Reveal in Explorer"
                      sub={clip.path}
                      align="left"
                      wrapperClass="w-full"
                    >
                      <button
                        onClick={() =>
                          revealInExplorer(clip.path).catch((e) =>
                            toastError(errorMessage(e)),
                          )
                        }
                        className="block w-full text-left text-[10px] text-t-muted truncate hover:text-t-text hover:underline underline-offset-2"
                      >
                        {clip.name}
                      </button>
                    </Tip>
                  ))}
                </div>
              </div>
            )}
          </div>
        );
      })}
      <div className="flex-1" />
      <Tip text="Settings (Ctrl+,)" align="left" wrapperClass="mx-auto">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          aria-label="Open Settings"
          onClick={() => openSettingsWindow().catch((e) => toastError(errorMessage(e)))}
        >
          <Settings className="h-4 w-4" />
        </Button>
      </Tip>
    </aside>
  );
}
