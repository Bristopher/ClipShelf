import { useEffect, useMemo, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { Folder, Music, RotateCcw, X } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { getVersion } from "@tauri-apps/api/app";
import {
  checkUpdateStatus,
  getMonitorCount,
  installUpdate,
  openReleasesPage,
  resetWindow,
} from "@/lib/commands";
import { ThemePanel } from "@/components/ThemePanel";
import { KeybindInput } from "@/components/KeybindInput";
import { SaveClipCalibration } from "@/components/SaveClipCalibration";
import { useObsStatus } from "@/hooks/useObsStatus";
import { errorMessage, toastError } from "@/lib/toast";
import { allThemes, resolveFlashTheme } from "@/lib/themes";
import type { AppConfig, UpdateStatus } from "@/types";

interface SettingsFormProps {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
}

/**
 * MediaStopper-style inline update flow: "Check for updates" button → result
 * card in place (no native popups). An available update shows a one-click
 * "Install vX & relaunch" (Velopack installs) or "Open releases page"
 * (portable/dev builds).
 */
function UpdateChecker() {
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [result, setResult] = useState<UpdateStatus | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);

  const checkNow = async () => {
    setChecking(true);
    setResult(null);
    setInstallError(null);
    try {
      setResult(await checkUpdateStatus());
    } catch (e) {
      setResult({ status: "error", current: "", canInstall: false, message: errorMessage(e) });
    } finally {
      setChecking(false);
    }
  };

  const install = async () => {
    setInstalling(true);
    setInstallError(null);
    try {
      await installUpdate(); // success = the app restarts out from under us
    } catch (e) {
      setInstallError(errorMessage(e));
      setInstalling(false);
    }
  };

  return (
    <div className="space-y-2">
      <Button
        variant="outline"
        size="sm"
        className="h-7 text-xs"
        disabled={checking || installing}
        onClick={checkNow}
      >
        {checking ? "Checking…" : "Check for updates"}
      </Button>

      {result?.status === "update" && (
        <div className="rounded-md border border-t-border bg-hover/40 p-2.5 space-y-2">
          <p className="text-xs font-semibold">
            Update available — {result.latest}{" "}
            <span className="font-normal text-t-muted">
              (you have v{result.current})
            </span>
          </p>
          {result.canInstall ? (
            <Button
              size="sm"
              className="h-7 text-xs"
              disabled={installing}
              onClick={install}
            >
              {installing
                ? "Downloading & installing…"
                : `Install ${result.latest} & relaunch`}
            </Button>
          ) : (
            <>
              <p className="text-[10px] text-t-muted">
                This build can't update itself (portable/dev) — grab the new
                version from the releases page.
              </p>
              <Button
                size="sm"
                variant="outline"
                className="h-7 text-xs"
                onClick={() => openReleasesPage().catch((e) => toastError(errorMessage(e)))}
              >
                Open releases page
              </Button>
            </>
          )}
          {installError && (
            <p className="text-[10px] text-red-500">
              Install failed: {installError} — opened the releases page for a
              manual download.
            </p>
          )}
        </div>
      )}
      {result?.status === "current" && (
        <p className="text-xs text-t-muted">
          You're on the latest version (v{result.current}).
        </p>
      )}
      {result?.status === "error" && (
        <p className="text-xs text-red-500">
          Couldn't check for updates: {result.message}
        </p>
      )}
    </div>
  );
}

/** All global-hotkey fields, for duplicate detection. */
const BIND_FIELDS = [
  ["g1_bind", "G1"],
  ["g2_bind", "G2"],
  ["g3_bind", "G3"],
  ["rename_bind", "Rename"],
  ["save_clip_bind", "Save clip"],
  ["count_up_bind", "Count-up"],
  ["undo_bind", "Undo"],
  ["overlay_bind", "Overlay toggle"],
] as const;

type BindField = (typeof BIND_FIELDS)[number][0];

/** Connection pill for the OBS WebSocket section. */
function ObsStatusPill() {
  const obs = useObsStatus();
  const [cls, label] =
    obs.status === "connected"
      ? ["bg-green-500/15 text-green-400 border-green-500/40", "Connected"]
      : obs.status === "connecting"
        ? ["bg-amber-500/15 text-amber-400 border-amber-500/40", "Connecting..."]
        : obs.status === "reconnecting"
          ? [
              "bg-amber-500/15 text-amber-400 border-amber-500/40",
              `Reconnecting (attempt ${obs.attempt})...`,
            ]
          : obs.status === "disabled"
            ? ["bg-panel text-t-muted border-t-border", "Off"]
            : ["bg-red-500/15 text-red-400 border-red-500/40", "Disconnected"];
  return (
    <span
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full border text-[10px] font-medium ${cls}`}
    >
      <span className="h-1.5 w-1.5 rounded-full bg-current" />
      {label}
    </span>
  );
}

export function SettingsForm({ config, onConfigChange }: SettingsFormProps) {
  const [version, setVersion] = useState<string>("");
  const [monitorCount, setMonitorCount] = useState(1);

  useEffect(() => {
    getVersion().then(setVersion).catch(console.error);
    getMonitorCount().then(setMonitorCount).catch(console.error);
  }, []);

  // Keybind conflict detection: same combo bound to two actions means one of
  // them silently loses at RegisterHotKey time. Flag it inline.
  const bindConflicts = useMemo(() => {
    const byCombo = new Map<string, string[]>();
    for (const [field, label] of BIND_FIELDS) {
      const v = (config[field] || "").trim().toLowerCase();
      if (!v) continue;
      byCombo.set(v, [...(byCombo.get(v) ?? []), label]);
    }
    const conflicts = new Map<BindField, string[]>();
    for (const [field] of BIND_FIELDS) {
      const v = (config[field] || "").trim().toLowerCase();
      if (!v) continue;
      const users = byCombo.get(v) ?? [];
      if (users.length > 1) conflicts.set(field, users);
    }
    return conflicts;
  }, [config]);

  const conflictNote = (field: BindField) => {
    const users = bindConflicts.get(field);
    if (!users) return null;
    const own = BIND_FIELDS.find(([f]) => f === field)?.[1];
    const others = users.filter((u) => u !== own);
    return (
      <p className="text-[10px] text-red-400">
        Same key as {others.join(", ")} — only one of them will work.
      </p>
    );
  };

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
        <div className="flex items-center justify-between">
          <div className="pr-2">
            <Label className="text-xs">Hold-to-click-through</Label>
            <p className="text-[10px] text-t-muted">
              While the key below is held, clicks pass through the window to
              whatever is underneath — no minimizing needed.
            </p>
          </div>
          <Switch
            checked={config.click_through_enabled}
            onCheckedChange={(v) => update({ click_through_enabled: v })}
          />
        </div>
        {config.click_through_enabled && (
          <div className="space-y-1">
            <Label className="text-xs">Click-through hold key</Label>
            <select
              value={config.click_through_key}
              onChange={(e) => update({ click_through_key: e.target.value })}
              className="w-full text-xs h-8 px-2 rounded bg-panel border border-t-border"
            >
              <option value="ctrl">Ctrl</option>
              <option value="alt">Alt</option>
              <option value="shift">Shift</option>
            </select>
            <p className="text-[10px] text-t-muted">
              Heads-up: while held, that key&apos;s +Click actions inside the
              app can&apos;t be reached (e.g. Ctrl+Click to play a clip) —
              pick Alt or Shift if you use those.
            </p>
          </div>
        )}
        <div className="flex items-center justify-between">
          <div className="pr-2">
            <Label className="text-xs">Start with Windows</Label>
            <p className="text-[10px] text-t-muted">
              Launch ClipShelf automatically when you log in.
            </p>
          </div>
          <Switch
            checked={config.autostart_enabled}
            onCheckedChange={(v) => update({ autostart_enabled: v })}
          />
        </div>
      </section>

      <Separator />

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Window</h3>
        <div className="flex items-center justify-between">
          <div className="pr-2">
            <Label className="text-xs">Remember position &amp; size</Label>
            <p className="text-[10px] text-t-muted">
              Reopen where you left the window. Off = always open at the
              default position below.
            </p>
          </div>
          <Switch
            checked={config.remember_window_layout}
            onCheckedChange={(v) => update({ remember_window_layout: v })}
          />
        </div>
        <div className="space-y-1">
          <Label className="text-xs">Default open position</Label>
          <div className="flex gap-2">
            <select
              value={Math.min(config.default_monitor, monitorCount)}
              onChange={(e) => update({ default_monitor: Number(e.target.value) })}
              className="flex-1 text-xs h-8 px-2 rounded bg-panel border border-t-border"
            >
              {Array.from({ length: monitorCount }, (_, i) => i + 1).map((n) => (
                <option key={n} value={n}>
                  Monitor {n}
                </option>
              ))}
            </select>
            <select
              value={config.default_anchor}
              onChange={(e) => update({ default_anchor: e.target.value })}
              className="flex-1 text-xs h-8 px-2 rounded bg-panel border border-t-border"
            >
              <option value="top-left">Top left</option>
              <option value="top-right">Top right</option>
              <option value="bottom-left">Bottom left</option>
              <option value="bottom-right">Bottom right</option>
              <option value="center">Center</option>
            </select>
          </div>
          <p className="text-[10px] text-t-muted">
            Used on launch when nothing is remembered, and by the Reset
            button.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="h-8 text-xs gap-1.5"
          onClick={() => resetWindow().catch(console.error)}
        >
          <RotateCcw className="h-3 w-3" />
          Reset window to default position
        </Button>
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
          {conflictNote("save_clip_bind")}
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
            {conflictNote(field)}
          </div>
        ))}
        <div className="space-y-1">
          <Label className="text-xs">Count-up stopwatch</Label>
          <KeybindInput
            value={config.count_up_bind}
            onChange={(v) => update({ count_up_bind: v })}
          />
          {conflictNote("count_up_bind")}
          <p className="text-[10px] text-t-muted">
            Press once to start counting up from 0. Press again to reset and
            stop. Press again to start over.
          </p>
        </div>
        <div className="space-y-1">
          <Label className="text-xs">Undo last move/rename</Label>
          <KeybindInput
            value={config.undo_bind}
            onChange={(v) => update({ undo_bind: v })}
          />
          {conflictNote("undo_bind")}
          <p className="text-[10px] text-t-muted">
            Puts the last clip back where it was (works for mis-pressed
            G-keys and renames). This is a GLOBAL hotkey — avoid Ctrl+Z or it
            will swallow undo in every other app. Try something like
            Ctrl+Alt+Z or a spare G-key.
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
        {(
          [
            ["clip_save_sound_custom", "Custom clip-save sound"],
            ["error_sound_custom", "Custom error sound"],
          ] as const
        ).map(([field, label]) => (
          <div key={field} className="space-y-1">
            <Label className="text-xs">{label}</Label>
            <div className="flex gap-2 items-center">
              <Input
                value={config[field] ?? ""}
                readOnly
                placeholder="Default sound"
                className="text-xs h-8 flex-1"
              />
              <Button
                variant="outline"
                size="icon"
                className="h-8 w-8 shrink-0"
                title="Pick an audio file (wav / mp3 / ogg / flac)"
                onClick={async () => {
                  try {
                    const selected = await open({
                      filters: [
                        {
                          name: "Audio",
                          extensions: ["wav", "mp3", "ogg", "flac"],
                        },
                      ],
                    });
                    if (typeof selected === "string") {
                      update({ [field]: selected });
                    }
                  } catch (e) {
                    toastError(errorMessage(e));
                  }
                }}
              >
                <Music className="h-4 w-4" />
              </Button>
              {config[field] && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 shrink-0"
                  title="Reset to the default sound"
                  onClick={() => update({ [field]: null })}
                >
                  <X className="h-4 w-4" />
                </Button>
              )}
            </div>
          </div>
        ))}
        <div className="space-y-1">
          <Label className="text-xs">
            Black-screen warning threshold ({config.small_file_warn_mb} MB)
          </Label>
          <Input
            type="number"
            min={0}
            max={500}
            step={0.5}
            value={config.small_file_warn_mb}
            className="text-xs h-8"
            onChange={(e) =>
              update({
                small_file_warn_mb: Math.max(0, Number(e.target.value) || 0),
              })
            }
          />
          <p className="text-[10px] text-t-muted">
            Clips smaller than this are flagged as possible black screens and
            play the error sound. Depends on your bitrate and replay length.
          </p>
        </div>
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
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-semibold">OBS WebSocket</h3>
          <ObsStatusPill />
        </div>
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

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Game detection</h3>
        <div className="flex items-center justify-between">
          <Label className="text-xs">Game detection</Label>
          <Switch
            checked={config.game_detection_enabled}
            onCheckedChange={(v) => update({ game_detection_enabled: v })}
          />
        </div>
        <div className="flex items-center justify-between">
          <div className="pr-2">
            <Label className="text-xs">
              Write game/rating/description into file properties (visible in
              Explorer)
            </Label>
          </div>
          <Switch
            checked={config.write_file_properties}
            disabled={!config.game_detection_enabled}
            onCheckedChange={(v) => update({ write_file_properties: v })}
          />
        </div>
        <div className="space-y-1">
          <Label className="text-xs">Day starts at (hour, 0–23)</Label>
          <Input
            type="number"
            min={0}
            max={23}
            value={config.day_rollover_hour}
            className="text-xs h-8"
            onChange={(e) =>
              update({
                day_rollover_hour: Math.min(
                  23,
                  Math.max(0, Number(e.target.value) || 0),
                ),
              })
            }
          />
          <p className="text-[10px] text-t-muted">
            History and daily stats roll over at this hour — default 4 AM for
            late-night sessions. (takes effect with the History panel)
          </p>
        </div>
        <GameOverridesEditor config={config} onConfigChange={onConfigChange} />
      </section>

      <Separator />

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Updates</h3>
        <div className="flex items-center justify-between">
          <div className="pr-2">
            <Label className="text-xs">Check for updates on launch</Label>
            <p className="text-[10px] text-t-muted">
              Never installs silently — a found update always asks first.
            </p>
          </div>
          <Switch
            checked={config.check_updates}
            onCheckedChange={(v) => update({ check_updates: v })}
          />
        </div>
        <UpdateChecker />
      </section>

      <Separator />

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Overlay</h3>
        <div className="flex items-center justify-between">
          <div className="pr-2">
            <Label className="text-xs">Enable in-game overlay</Label>
            <p className="text-[10px] text-t-muted">
              A small menu over your game to sort/rate/label/describe the
              latest clip without alt-tabbing.
            </p>
          </div>
          <Switch
            checked={config.overlay_enabled}
            onCheckedChange={(v) => update({ overlay_enabled: v })}
          />
        </div>
        {config.overlay_enabled && (
          <>
            <div className="space-y-1">
              <Label className="text-xs">Overlay toggle hotkey</Label>
              <KeybindInput
                value={config.overlay_bind}
                onChange={(v) => update({ overlay_bind: v })}
              />
              {conflictNote("overlay_bind")}
              <p className="text-[10px] text-t-muted">
                Global hotkey that opens/closes the overlay over your game.
              </p>
            </div>
            <div className="flex items-center justify-between">
              <div className="pr-2">
                <Label className="text-xs">Allow typing in overlay</Label>
                <p className="text-[10px] text-t-muted">
                  Lets you type a custom label, description, or game name
                  in-game via a low-level keyboard hook, without alt-tabbing.
                </p>
              </div>
              <Switch
                checked={config.overlay_typing_enabled}
                onCheckedChange={(v) => update({ overlay_typing_enabled: v })}
              />
            </div>
            <PresetsEditor
              label="Label presets"
              placeholder="e.g. Clutch"
              values={config.label_presets}
              onChange={(v) => update({ label_presets: v })}
            />
            <PresetsEditor
              label="Description presets"
              placeholder="e.g. GG ez"
              values={config.description_presets}
              onChange={(v) => update({ description_presets: v })}
            />
          </>
        )}
      </section>

      <Separator />

      <section className="pt-1 pb-2">
        <p className="text-[11px] text-t-muted text-center">
          ClipShelf {version ? `v${version}` : ""}
        </p>
      </section>
    </div>
  );
}

/** Editable table of exe -> display-name corrections that always win over detection. */
function GameOverridesEditor({ config, onConfigChange }: SettingsFormProps) {
  const [newExe, setNewExe] = useState("");
  const [newName, setNewName] = useState("");

  const update = (partial: Partial<AppConfig>) => {
    onConfigChange({ ...config, ...partial });
  };

  const overrides = config.game_overrides ?? [];

  const setOverrideName = (exe: string, name: string) => {
    update({
      game_overrides: overrides.map((o) =>
        o.exe.toLowerCase() === exe.toLowerCase() ? { ...o, name } : o,
      ),
    });
  };

  const removeOverride = (exe: string) => {
    update({
      game_overrides: overrides.filter(
        (o) => o.exe.toLowerCase() !== exe.toLowerCase(),
      ),
    });
  };

  const addOverride = () => {
    const exe = newExe.trim();
    const name = newName.trim();
    if (!exe || !name) return;
    const existingIdx = overrides.findIndex(
      (o) => o.exe.toLowerCase() === exe.toLowerCase(),
    );
    const next =
      existingIdx >= 0
        ? overrides.map((o, i) => (i === existingIdx ? { exe, name } : o))
        : [...overrides, { exe, name }];
    update({ game_overrides: next });
    setNewExe("");
    setNewName("");
  };

  return (
    <div className="space-y-1.5">
      <Label className="text-xs">Overrides</Label>
      {overrides.length > 0 && (
        <div className="space-y-1.5">
          {overrides.map((o) => (
            <div key={o.exe} className="flex gap-2 items-center">
              <Input
                value={o.exe}
                readOnly
                className="text-xs h-8 flex-1 text-t-muted"
              />
              <Input
                value={o.name}
                className="text-xs h-8 flex-1"
                onChange={(e) => setOverrideName(o.exe, e.target.value)}
              />
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 shrink-0"
                title="Remove override"
                onClick={() => removeOverride(o.exe)}
              >
                <X className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
      )}
      <div className="flex gap-2 items-center">
        <Input
          value={newExe}
          placeholder="game.exe"
          className="text-xs h-8 flex-1"
          onChange={(e) => setNewExe(e.target.value)}
        />
        <Input
          value={newName}
          placeholder="Display name"
          className="text-xs h-8 flex-1"
          onChange={(e) => setNewName(e.target.value)}
        />
        <Button
          variant="outline"
          size="sm"
          className="h-8 text-xs shrink-0"
          onClick={addOverride}
        >
          Add
        </Button>
      </div>
      <p className="text-[10px] text-t-muted">
        When a game is detected wrong, corrections you save here (or via
        Remember) always win.
      </p>
    </div>
  );
}

/** Chip list editor for the overlay's numbered label/description presets. */
function PresetsEditor({
  label,
  placeholder,
  values,
  onChange,
}: {
  label: string;
  placeholder: string;
  values: string[];
  onChange: (values: string[]) => void;
}) {
  const [draft, setDraft] = useState("");

  const add = () => {
    const v = draft.trim();
    if (!v || values.includes(v)) return;
    onChange([...values, v]);
    setDraft("");
  };

  const remove = (v: string) => onChange(values.filter((x) => x !== v));

  return (
    <div className="space-y-1.5">
      <Label className="text-xs">{label}</Label>
      {values.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {values.map((v) => (
            <span
              key={v}
              className="inline-flex items-center gap-1 pl-2 pr-1 py-0.5 rounded-full bg-panel border border-t-border text-[11px]"
            >
              {v}
              <button
                type="button"
                onClick={() => remove(v)}
                title="Remove"
                className="p-0.5 rounded-full text-t-muted hover:text-t-text hover:bg-hover"
              >
                <X className="h-3 w-3" />
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="flex gap-2 items-center">
        <Input
          value={draft}
          placeholder={placeholder}
          className="text-xs h-8 flex-1"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              add();
            }
          }}
        />
        <Button
          variant="outline"
          size="sm"
          className="h-8 text-xs shrink-0"
          onClick={add}
        >
          Add
        </Button>
      </div>
      <p className="text-[10px] text-t-muted">
        Numbered 1-9 in the overlay menu, in this order. Press 0 for custom
        text instead.
      </p>
    </div>
  );
}
