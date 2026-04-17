import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { wipeLog, restoreLog } from "@/lib/commands";
import type { LogEntry } from "@/types";

interface BottomBarProps {
  mode: "rename" | "sort";
  autoWipe: boolean;
  onAutoWipeChange: (value: boolean) => void;
  onWipe: () => void;
  onRestore: (entries: LogEntry[]) => void;
}

export function BottomBar({
  mode,
  autoWipe,
  onAutoWipeChange,
  onWipe,
  onRestore,
}: BottomBarProps) {
  const handleWipe = async () => {
    await wipeLog();
    onWipe();
  };

  const handleRestore = async () => {
    const restored = await restoreLog();
    onRestore(restored);
  };

  return (
    <div className="border-t border-border px-3 py-1.5 flex items-center gap-3 text-xs">
      <span className="text-muted-foreground">
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
        <Switch
          id="auto-wipe"
          checked={autoWipe}
          onCheckedChange={onAutoWipeChange}
          className="scale-75"
        />
        <Label htmlFor="auto-wipe" className="text-xs text-muted-foreground cursor-pointer">
          Auto-Wipe
        </Label>
      </div>
    </div>
  );
}
