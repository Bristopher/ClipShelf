import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Folder, Sparkles } from "lucide-react";
import { updateConfig } from "@/lib/commands";
import logoUrl from "@/assets/gkey-logo.png";
import type { AppConfig } from "@/types";

interface FirstRunDialogProps {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
}

export function FirstRunDialog({ config, onConfigChange }: FirstRunDialogProps) {
  const [folder, setFolder] = useState("");
  const [g1, setG1] = useState(config.g1_bind);
  const [g2, setG2] = useState(config.g2_bind);
  const [g3, setG3] = useState(config.g3_bind);
  const [renameBind, setRenameBind] = useState(config.rename_bind);
  const [saving, setSaving] = useState(false);

  const pickFolder = async () => {
    const selected = await open({ directory: true });
    if (typeof selected === "string") {
      setFolder(selected);
    }
  };

  const finish = async () => {
    if (!folder) return;
    setSaving(true);
    try {
      const updated = await updateConfig({
        videos_folder: folder,
        g1_bind: g1,
        g2_bind: g2,
        g3_bind: g3,
        rename_bind: renameBind,
      });
      onConfigChange(updated);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
      <div className="w-full max-w-md rounded-lg border border-t-border bg-panel shadow-2xl overflow-hidden">
        <div className="flex items-center gap-3 px-5 py-4 border-b border-t-border">
          <img src={logoUrl} alt="" className="h-7 w-7 rounded" />
          <div>
            <h2 className="text-sm font-semibold flex items-center gap-1.5">
              First-time setup
              <Sparkles className="h-3.5 w-3.5 text-t-muted" />
            </h2>
            <p className="text-[11px] text-t-muted">
              Tell GKey Mover where your clips live.
            </p>
          </div>
        </div>

        <div className="px-5 py-4 space-y-4 max-h-[70vh] overflow-y-auto">
          <p className="text-xs text-t-muted leading-relaxed">
            GKey Mover watches a folder for new clips from OBS or ShadowPlay. Any
            video that lands there will appear here, ready to sort or rename with
            a hotkey.
          </p>

          <div className="space-y-1.5">
            <Label className="text-xs font-semibold">Clips folder</Label>
            <div className="flex gap-2">
              <Input
                value={folder}
                placeholder="C:\Users\you\Videos\Replays"
                readOnly
                className="text-xs h-8 flex-1"
              />
              <Button
                variant="outline"
                size="icon"
                className="h-8 w-8 shrink-0"
                onClick={pickFolder}
              >
                <Folder className="h-4 w-4" />
              </Button>
            </div>
            <p className="text-[10px] text-t-muted">
              Point this at OBS's "Recording Path" or ShadowPlay's "Gallery folder".
            </p>
          </div>

          <div className="space-y-2 pt-2 border-t border-t-border">
            <Label className="text-xs font-semibold">Hotkeys (optional)</Label>
            <p className="text-[10px] text-t-muted">
              Defaults shown — tweak later in Settings if they clash.
            </p>
            <div className="grid grid-cols-2 gap-2">
              <KeyRow label="G1" value={g1} onChange={setG1} />
              <KeyRow label="G2" value={g2} onChange={setG2} />
              <KeyRow label="G3" value={g3} onChange={setG3} />
              <KeyRow label="Rename" value={renameBind} onChange={setRenameBind} />
            </div>
          </div>
        </div>

        <div className="px-5 py-3 border-t border-t-border flex justify-end">
          <Button
            onClick={finish}
            disabled={!folder || saving}
            className="h-8 text-xs"
          >
            {saving ? "Saving..." : "Let's go"}
          </Button>
        </div>
      </div>
    </div>
  );
}

function KeyRow({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="space-y-1">
      <Label className="text-[10px] text-t-muted">{label}</Label>
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="text-[11px] h-7 font-mono"
      />
    </div>
  );
}
