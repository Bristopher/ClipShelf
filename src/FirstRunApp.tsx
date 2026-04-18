import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { KeybindInput } from "@/components/KeybindInput";
import { Folder, Sparkles } from "lucide-react";
import { getConfig, updateConfig } from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { refreshSystemMode } from "@/lib/systemTheme";
import { useTheme } from "@/hooks/useTheme";
import { WindowChrome } from "@/components/WindowChrome";
import logoUrl from "@/assets/gkey-logo.png";
import type { AppConfig } from "@/types";

export function FirstRunApp() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [folder, setFolder] = useState("");
  const [g1, setG1] = useState("");
  const [g2, setG2] = useState("");
  const [g3, setG3] = useState("");
  const [renameBind, setRenameBind] = useState("");
  const [saveClipBind, setSaveClipBind] = useState("");
  const [saving, setSaving] = useState(false);
  useTheme(config);

  useEffect(() => {
    getConfig().then((cfg) => {
      setConfig(cfg);
      setFolder(cfg.videos_folder);
      setG1(cfg.g1_bind);
      setG2(cfg.g2_bind);
      setG3(cfg.g3_bind);
      setRenameBind(cfg.rename_bind);
      setSaveClipBind(cfg.save_clip_bind);
    });
    refreshSystemMode().catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen<AppConfig>(EVENTS.CONFIG_CHANGED, (e) => {
      setConfig(e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const pickFolder = async () => {
    const selected = await open({ directory: true });
    if (typeof selected === "string") setFolder(selected);
  };

  const finish = async () => {
    if (!folder) return;
    setSaving(true);
    try {
      await updateConfig({
        videos_folder: folder,
        g1_bind: g1,
        g2_bind: g2,
        g3_bind: g3,
        rename_bind: renameBind,
        save_clip_bind: saveClipBind,
      });
      await getCurrentWindow().hide();
    } finally {
      setSaving(false);
    }
  };

  if (!config) {
    return (
      <div className="flex flex-col h-screen bg-app-bg text-t-text">
        <WindowChrome title="Setup" />
        <div className="flex-1 flex items-center justify-center">
          <p className="text-sm text-t-muted">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen bg-app-bg text-t-text">
      <WindowChrome title="Setup" />
      <div className="flex-1 overflow-y-auto">
      <div className="max-w-md mx-auto px-6 py-6 space-y-5">
        <header className="flex items-center gap-3">
          <img src={logoUrl} alt="" className="h-10 w-10 rounded" />
          <div>
            <h1 className="text-base font-semibold flex items-center gap-2">
              First-time setup
              <Sparkles className="h-4 w-4 text-t-muted" />
            </h1>
            <p className="text-[11px] text-t-muted">
              Tell GKey Mover where your clips live.
            </p>
          </div>
        </header>

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
              placeholder="C:\\Users\\you\\Videos\\Replays"
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
          <Label className="text-xs font-semibold">Capture app hotkey</Label>
          <p className="text-[10px] text-t-muted">
            The key you press in OBS / ShadowPlay to save a clip. Optional —
            used later to detect "hit save but no file appeared" errors.
          </p>
          <KeyRow
            label="Save clip"
            value={saveClipBind}
            onChange={setSaveClipBind}
          />
        </div>

        <div className="space-y-2 pt-2 border-t border-t-border">
          <Label className="text-xs font-semibold">GKey Mover hotkeys</Label>
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

        <div className="pt-2 flex justify-end">
          <Button onClick={finish} disabled={!folder || saving} className="h-9">
            {saving ? "Saving..." : "Let's go"}
          </Button>
        </div>
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
      <KeybindInput value={value} onChange={onChange} />
    </div>
  );
}
