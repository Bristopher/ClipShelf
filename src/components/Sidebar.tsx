import { Settings } from "lucide-react";
import { emit } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { openSettingsWindow, pressGkey } from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { errorMessage, toastError } from "@/lib/toast";
import type { AppConfig } from "@/types";

interface SidebarProps {
  config: AppConfig;
}

/** Compact button tag from the configured folder name (max 5 chars). */
function tagFor(folderName: string, fallback: string): string {
  const name = folderName.trim();
  if (!name) return fallback;
  return name.length <= 5 ? name.toUpperCase() : name.slice(0, 4).toUpperCase() + "…";
}

export function Sidebar({ config }: SidebarProps) {
  const sorting = !config.disable_file_movesorting;
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
      {gkeys.map((g) => (
        <button
          key={g.key}
          // G4 opens the rename dialog (press_gkey rejects key 4 — it's not
          // a move bind). The dialog listens for hotkey-triggered key=4.
          onClick={() =>
            g.key === 4
              ? emit(EVENTS.HOTKEY_TRIGGERED, { key: 4 }).catch(console.error)
              : pressGkey(g.key).catch((e) => toastError(errorMessage(e)))
          }
          style={{ backgroundColor: g.accent }}
          title={
            g.key === 4
              ? "Rename the current clip"
              : sorting
                ? `Move current clip to "${g.folder || g.label}"`
                : `Tag current clip as "${g.folder || g.label}" (rename only)`
          }
          aria-label={g.key === 4 ? "Rename clip" : `Sort clip to ${g.folder || g.label}`}
          className="rounded px-1.5 py-1.5 text-white text-[10px] font-bold flex flex-col items-center gap-0 transition-[filter,transform] hover:brightness-110 active:scale-95"
        >
          <span>{g.label}</span>
          <span className="text-[8px] opacity-80 leading-tight max-w-full truncate">
            {g.tag}
          </span>
        </button>
      ))}
      <div className="flex-1" />
      <Button
        variant="ghost"
        size="icon"
        className="mx-auto h-7 w-7"
        title="Settings"
        aria-label="Open Settings"
        onClick={() => openSettingsWindow().catch((e) => toastError(errorMessage(e)))}
      >
        <Settings className="h-4 w-4" />
      </Button>
    </aside>
  );
}
