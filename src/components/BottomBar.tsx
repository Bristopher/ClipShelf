import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Timer as TimerIcon } from "lucide-react";
import { wipeLog, restoreLog, toggleCountUp } from "@/lib/commands";
import { useCountUp } from "@/hooks/useCountUp";
import type { LogEntry } from "@/types";

interface BottomBarProps {
  mode: "rename" | "sort";
  autoWipe: boolean;
  onAutoWipeChange: (value: boolean) => void;
  onWipe: () => void;
  onRestore: (entries: LogEntry[]) => void;
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
}: BottomBarProps) {
  const countUp = useCountUp();

  const handleWipe = async () => {
    await wipeLog();
    onWipe();
  };

  const handleRestore = async () => {
    const restored = await restoreLog();
    onRestore(restored);
  };

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
          onClick={() => toggleCountUp().catch(console.error)}
          title="Toggle count-up stopwatch (start ↔ reset)"
        >
          <TimerIcon className="h-3 w-3" />
          {countUp.running ? "Reset" : "Start ↑"}
        </Button>
        <span
          className={`font-mono text-[11px] tabular-nums px-1.5 py-0.5 rounded ${
            countUp.running ? "text-t-text bg-panel" : "text-t-muted"
          }`}
          title="Count-up stopwatch"
        >
          {fmt(countUp.elapsedSecs)}
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
