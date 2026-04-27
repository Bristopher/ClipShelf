import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { Folder } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { getVersion } from "@tauri-apps/api/app";
import { ThemePanel } from "@/components/ThemePanel";
import { KeybindInput } from "@/components/KeybindInput";
import { SaveClipCalibration } from "@/components/SaveClipCalibration";
import { allThemes, resolveFlashTheme } from "@/lib/themes";
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

  // Updates the in-memory draft only — persistence happens when the user
  // clicks Save in the parent's button bar. ThemePanel is the one
  // exception: its actions write to disk immediately because theme state
  // (custom themes, import/export) is heavy enough that a draft model
  // would be confusing.
  const update = (partial: Partial<AppConfig>) => {
    onConfigChange({ ...config, ...partial });
  };

  const pickFolder = async (field: keyof AppConfig) => {
    const selected = await open({ directory: true });
    if (selected) {
      update({ [field]: selected });
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
        <div className="space-y-1">
          <Label className="text-xs">Capture app save-clip hotkey</Label>
          <KeybindInput
            value={config.save_clip_bind}
            onChange={(v) => update({ save_clip_bind: v })}
          />
          <p className="text-[10px] text-t-muted">
            Whatever key you press in OBS / ShadowPlay to save a clip. Used as
            a watcher health probe — if no clip arrives within the timeout
            below, we restart the watcher and rescan the folder.
          </p>
        </div>
        <div className="space-y-1">
          <Label className="text-xs">
            Health-check timeout ({config.save_clip_health_check_timeout_secs}s)
          </Label>
          <Input
            type="number"
            min={1}
            max={60}
            value={config.save_clip_health_check_timeout_secs}
            className="text-xs h-8"
            onChange={(e) =>
              update({
                save_clip_health_check_timeout_secs: Math.max(
                  1,
                  Number(e.target.value) || 5,
                ),
              })
            }
          />
          <p className="text-[10px] text-t-muted">
            Hardware-dependent: SSDs flush in ~1s, slow HDDs or long replay
            buffers can take 5-10s. Click below to measure yours.
          </p>
        </div>
        <SaveClipCalibration config={config} onConfigChange={onConfigChange} />
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
            <KeybindInput
              value={config[field]}
              onChange={(v) => update({ [field]: v })}
            />
          </div>
        ))}
        <div className="space-y-1">
          <Label className="text-xs">Count-up stopwatch</Label>
          <KeybindInput
            value={config.count_up_bind}
            onChange={(v) => update({ count_up_bind: v })}
          />
          <p className="text-[10px] text-t-muted">
            Press once to start counting up from 0. Press again to reset and
            stop. Press again to start over.
          </p>
        </div>
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
        <h3 className="text-sm font-semibold">Timer / Auto-Wipe</h3>
        <p className="text-[10px] text-t-muted">
          When a clip arrives (or you hit Start), a countdown runs. Match this
          to your OBS replay-buffer length so you know when to press save.
        </p>
        <div className="flex items-center justify-between">
          <Label className="text-xs">Timer enabled</Label>
          <Switch
            checked={config.timer_enabled}
            onCheckedChange={(v) => update({ timer_enabled: v })}
          />
        </div>
        <div className="space-y-1">
          <Label className="text-xs">
            Auto-wipe time ({Math.floor(config.timer_duration_ms / 1000)}s)
          </Label>
          <Input
            type="number"
            min={5}
            max={3600}
            value={Math.floor(config.timer_duration_ms / 1000)}
            className="text-xs h-8"
            onChange={(e) =>
              update({ timer_duration_ms: Number(e.target.value) * 1000 })
            }
          />
        </div>
        <div className="flex items-center justify-between">
          <Label className="text-xs">Auto-wipe event log on expiry</Label>
          <Switch
            checked={config.auto_wipe_enabled}
            onCheckedChange={(v) => update({ auto_wipe_enabled: v })}
          />
        </div>
        <div className="flex items-center justify-between">
          <div className="pr-2">
            <Label className="text-xs">Flash window at ≤ 5s left</Label>
            <p className="text-[10px] text-t-muted">
              Swaps to a contrasting theme once per second so it's obvious the
              timer is about to expire.
            </p>
          </div>
          <Switch
            checked={config.timer_flash_enabled}
            onCheckedChange={(v) => update({ timer_flash_enabled: v })}
          />
        </div>
        {config.timer_flash_enabled && (
          <div className="space-y-1.5 pl-2 border-l-2 border-t-border">
            <div className="flex items-center justify-between">
              <div className="pr-2">
                <Label className="text-xs">Override flash theme</Label>
                <p className="text-[10px] text-t-muted">
                  Off = auto-pick (light → dark, dark → light). On lets you
                  choose any theme to swap to during flash.
                </p>
              </div>
              <Switch
                checked={config.timer_flash_theme_id != null}
                onCheckedChange={(v) =>
                  update({
                    timer_flash_theme_id: v
                      ? resolveFlashTheme(config).id
                      : null,
                  })
                }
              />
            </div>
            {config.timer_flash_theme_id != null && (
              <select
                value={config.timer_flash_theme_id}
                onChange={(e) =>
                  update({ timer_flash_theme_id: e.target.value })
                }
                className="w-full text-xs h-8 px-2 rounded bg-panel border border-t-border"
              >
                {allThemes(config)
                  .filter((t) => t.id !== "system")
                  .map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.name}
                    </option>
                  ))}
              </select>
            )}
          </div>
        )}
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
