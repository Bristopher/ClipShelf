import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Play, RotateCcw } from "lucide-react";
import {
  wipeLog,
  restoreLog,
  startUserTimer,
  resetUserTimer,
} from "@/lib/commands";
import { useTimer } from "@/hooks/useTimer";
import { EVENTS } from "@/lib/events";
import type { LogEntry } from "@/types";

interface BottomBarProps {
  mode: "rename" | "sort";
  autoWipe: boolean;
  onAutoWipeChange: (value: boolean) => void;
  onWipe: () => void;
  onRestore: (entries: LogEntry[]) => void;
  configuredSecs: number;
}

function fmt(secs: number) {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

export function BottomBar({
  mode,
  autoWipe,
  onAutoWipeChange,
  onWipe,
  onRestore,
  configuredSecs,
}: BottomBarProps) {
  const userTimer = useTimer(configuredSecs, {
    tickEvent: EVENTS.USER_TIMER_TICK,
    expiredEvent: EVENTS.USER_TIMER_EXPIRED,
  });

  const handleWipe = async () => {
    await wipeLog();
    onWipe();
  };

  const handleRestore = async () => {
    const restored = await restoreLog();
    onRestore(restored);
  };

  const displaySecs = userTimer.running
    ? userTimer.remainingSecs
    : configuredSecs;

  return (
    <div className="border-t border-t-border px-3 py-1.5 flex items-center gap-3 text-xs">
      <span className="text-t-muted">
        Mode: {mode === "rename" ? "Rename Only" : "Folder Sort"}
      </span>

      <Separator orientation="vertical" className="h-4" />

      <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={handleWipe}>
        Wipe
      </Button>
      <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={handleRestore}>
        Restore
      </Button>

      <Separator orientation="vertical" className="h-4" />

      <div className="flex items-center gap-1.5">
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs gap-1"
          onClick={() => startUserTimer().catch(console.error)}
          title="Start the manual countdown"
        >
          <Play className="h-3 w-3" />
          Start
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs gap-1"
          onClick={() => resetUserTimer().catch(console.error)}
          title="Reset the manual countdown"
          disabled={!userTimer.running && userTimer.remainingSecs === configuredSecs}
        >
          <RotateCcw className="h-3 w-3" />
          Reset
        </Button>
        <span
          className={`font-mono text-[11px] tabular-nums px-1.5 py-0.5 rounded ${
            userTimer.running
              ? "text-t-text bg-panel"
              : "text-t-muted"
          }`}
          title="Manual countdown"
        >
          {fmt(displaySecs)}
        </span>
      </div>

      <Separator orientation="vertical" className="h-4" />

      <div className="flex items-center gap-1.5">
        <Switch
          id="auto-wipe"
          checked={autoWipe}
          onCheckedChange={onAutoWipeChange}
          className="scale-75"
        />
        <Label htmlFor="auto-wipe" className="text-xs text-t-muted cursor-pointer">
          Auto-Wipe
        </Label>
      </div>
    </div>
  );
}
