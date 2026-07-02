import { Settings } from "lucide-react";
import { emit } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { openSettingsWindow, pressGkey } from "@/lib/commands";
import { EVENTS } from "@/lib/events";

const gkeys = [
  { key: 1, label: "G1", tag: "!!", accent: "var(--t-g1-accent)" },
  { key: 2, label: "G2", tag: "CHKD", accent: "var(--t-g2-accent)" },
  { key: 3, label: "G3", tag: "!!!", accent: "var(--t-g3-accent)" },
  { key: 4, label: "G4", tag: "REN", accent: "var(--t-g4-accent)" },
] as const;

export function Sidebar() {
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
              : pressGkey(g.key)
          }
          style={{ backgroundColor: g.accent }}
          className="rounded px-1.5 py-1.5 text-white text-[10px] font-bold flex flex-col items-center gap-0 transition-[filter,transform] hover:brightness-110 active:scale-95"
        >
          <span>{g.label}</span>
          <span className="text-[8px] opacity-80 leading-tight">{g.tag}</span>
        </button>
      ))}
      <div className="flex-1" />
      <Button
        variant="ghost"
        size="icon"
        className="mx-auto h-7 w-7"
        onClick={() => openSettingsWindow().catch(console.error)}
      >
        <Settings className="h-4 w-4" />
      </Button>
    </aside>
  );
}
