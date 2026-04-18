import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { Folder } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { getVersion } from "@tauri-apps/api/app";
import { updateConfig } from "@/lib/commands";
import { ThemePanel } from "@/components/ThemePanel";
import type { AppConfig } from "@/types";

interface SettingsFormProps {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
}

export function SettingsForm({ config, onConfigChange }: SettingsFormProps) {
  const [version, setVersion] = useState<string>("");

  useEffect(() => {
    getVersion().then(setVersion).catch(console.error);
  }, []);

  const update = async (partial: Partial<AppConfig>) => {
    const updated = await updateConfig(partial);
    onConfigChange(updated);
  };

  const pickFolder = async (field: keyof AppConfig) => {
    const selected = await open({ directory: true });
    if (selected) {
      await update({ [field]: selected });
    }
  };

  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <h3 className="text-sm font-semibold">General</h3>
        <div className="space-y-1.5">
          <Label className="text-xs">Videos Folder</Label>
          <div className="flex gap-2">
            <Input
              value={config.videos_folder}
              readOnly
              className="text-xs h-8 flex-1"
            />
            <Button
              variant="outline"
              size="icon"
              className="h-8 w-8 shrink-0"
              onClick={() => pickFolder("videos_folder")}
            >
              <Folder className="h-4 w-4" />
            </Button>
          </div>
        </div>
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <Label className="text-xs">
              Window Opacity ({Math.round((config.window_opacity ?? 1) * 100)}%)
            </Label>
          </div>
          <input
            type="range"
            min="20"
            max="100"
            value={Math.round((config.window_opacity ?? 1) * 100)}
            onChange={(e) =>
              update({ window_opacity: Number(e.target.value) / 100 })
            }
            className="w-full h-1.5 rounded-full appearance-none cursor-pointer bg-secondary accent-primary"
          />
        </div>
        <div className="flex items-center justify-between">
          <Label className="text-xs">Full opacity on hover</Label>
          <Switch
            checked={config.hover_full_opacity}
            onCheckedChange={(v) => update({ hover_full_opacity: v })}
          />
        </div>
      </section>

      <Separator />

      <ThemePanel config={config} onConfigChange={onConfigChange} />

      <Separator />

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Hotkeys</h3>
        {(
          [
            ["g1_bind", "G1 Bind"],
            ["g2_bind", "G2 Bind"],
            ["g3_bind", "G3 Bind"],
            ["rename_bind", "Rename Bind"],
          ] as const
        ).map(([field, label]) => (
          <div key={field} className="space-y-1">
            <Label className="text-xs">{label}</Label>
            <Input
              value={config[field]}
              className="text-xs h-8"
              onChange={(e) => update({ [field]: e.target.value })}
            />
          </div>
        ))}
      </section>

      <Separator />

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Mode</h3>
        <div className="flex items-center justify-between">
          <Label className="text-xs">Rename Only (disable folder sorting)</Label>
          <Switch
            checked={config.disable_file_movesorting}
            onCheckedChange={(v) => update({ disable_file_movesorting: v })}
          />
        </div>
      </section>

      {!config.disable_file_movesorting && (
        <>
          <Separator />
          <section className="space-y-3">
            <h3 className="text-sm font-semibold">Sort Folders</h3>
            {(
              [
                ["g1_bind_folder_name", "G1 Folder"],
                ["g2_bind_folder_name", "G2 Folder"],
                ["g3_bind_folder_name", "G3 Folder"],
              ] as const
            ).map(([field, label]) => (
              <div key={field} className="space-y-1">
                <Label className="text-xs">{label}</Label>
                <Input
                  value={config[field]}
                  className="text-xs h-8"
                  onChange={(e) => update({ [field]: e.target.value })}
                />
              </div>
            ))}
          </section>
        </>
      )}

      <Separator />

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Sounds</h3>
        {(
          [
            ["clip_save_sound_enabled", "Clip Save Sound"],
            ["move_sound_enabled", "Move Sound"],
            ["error_sound_enabled", "Error Sound"],
          ] as const
        ).map(([field, label]) => (
          <div key={field} className="flex items-center justify-between">
            <Label className="text-xs">{label}</Label>
            <Switch
              checked={config[field]}
              onCheckedChange={(v) => update({ [field]: v })}
            />
          </div>
        ))}
      </section>

      <Separator />

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Timer</h3>
        <div className="flex items-center justify-between">
          <Label className="text-xs">Enabled</Label>
          <Switch
            checked={config.timer_enabled}
            onCheckedChange={(v) => update({ timer_enabled: v })}
          />
        </div>
        <div className="space-y-1">
          <Label className="text-xs">Duration (seconds)</Label>
          <Input
            type="number"
            value={Math.floor(config.timer_duration_ms / 1000)}
            className="text-xs h-8"
            onChange={(e) =>
              update({ timer_duration_ms: Number(e.target.value) * 1000 })
            }
          />
        </div>
        <div className="flex items-center justify-between">
          <Label className="text-xs">Auto-Wipe on Expiry</Label>
          <Switch
            checked={config.auto_wipe_enabled}
            onCheckedChange={(v) => update({ auto_wipe_enabled: v })}
          />
        </div>
      </section>

      <Separator />

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">OBS WebSocket</h3>
        <div className="flex items-center justify-between">
          <Label className="text-xs">Enabled</Label>
          <Switch
            checked={config.obs_websocket_enabled}
            onCheckedChange={(v) => update({ obs_websocket_enabled: v })}
          />
        </div>
        {config.obs_websocket_enabled && (
          <div className="space-y-1">
            <Label className="text-xs">Password</Label>
            <Input
              type="password"
              value={config.obs_websocket_password}
              className="text-xs h-8"
              onChange={(e) =>
                update({ obs_websocket_password: e.target.value })
              }
            />
          </div>
        )}
      </section>

      <Separator />

      <section className="pt-1 pb-2">
        <p className="text-[11px] text-t-muted text-center">
          GKey Mover {version ? `v${version}` : ""}
        </p>
      </section>
    </div>
  );
}
