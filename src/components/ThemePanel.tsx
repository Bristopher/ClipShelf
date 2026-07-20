import { useEffect, useMemo, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Tip } from "@/components/ui/tip";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Download, Upload, Save, Trash2, Palette } from "lucide-react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  allThemes,
  applyTheme,
  cloneTokens,
  isValidCssColor,
  resolveTheme,
  slugify,
  uniqueId,
  uniqueName,
  BUILTIN_THEMES,
  SYSTEM_THEME_ID,
} from "@/lib/themes";
import { exportTheme, importTheme, updateConfig } from "@/lib/commands";
import {
  THEME_TOKEN_ORDER,
  THEME_TOKEN_LABELS,
  type AppConfig,
  type Theme,
  type ThemeTokens,
  type ThemeTokenKey,
} from "@/types";

interface ThemePanelProps {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
}

type DraftState =
  | { kind: "clean" }
  | { kind: "editing"; tokens: ThemeTokens; sourceId: string };

export function ThemePanel({ config, onConfigChange }: ThemePanelProps) {
  const active = useMemo(() => resolveTheme(config), [config]);
  const [draft, setDraft] = useState<DraftState>({ kind: "clean" });
  const isSystem = config.active_theme_id === SYSTEM_THEME_ID;

  // When user switches active theme, reset any in-flight draft.
  useEffect(() => {
    setDraft({ kind: "clean" });
  }, [config.active_theme_id]);

  const shownTokens: ThemeTokens =
    draft.kind === "editing" ? draft.tokens : active.tokens;

  // Live-preview the draft to :root without persisting.
  useEffect(() => {
    if (draft.kind === "editing") {
      applyTheme({ id: "__draft", name: "Draft", builtin: false, tokens: draft.tokens });
    } else {
      applyTheme(active);
    }
  }, [draft, active]);

  const persist = async (partial: Partial<AppConfig>) => {
    const updated = await updateConfig(partial);
    onConfigChange(updated);
  };

  const onPickTheme = async (id: string | null) => {
    if (!id) return;
    await persist({ active_theme_id: id });
  };

  const beginEdit = () => {
    setDraft({
      kind: "editing",
      tokens: cloneTokens(active.tokens),
      sourceId: active.id,
    });
  };

  const cancelEdit = () => {
    setDraft({ kind: "clean" });
  };

  const setToken = (key: ThemeTokenKey, value: string) => {
    if (draft.kind !== "editing") return;
    setDraft({
      kind: "editing",
      sourceId: draft.sourceId,
      tokens: { ...draft.tokens, [key]: value },
    });
  };

  // Save draft back onto the same custom theme; built-ins can't be overwritten.
  const saveInPlace = async () => {
    if (draft.kind !== "editing") return;
    const target = config.themes.find((t) => t.id === draft.sourceId);
    if (!target) {
      // Source was a built-in — auto "save as new" to avoid silent clone-on-edit loss.
      return saveAsNew();
    }
    const next = config.themes.map((t) =>
      t.id === draft.sourceId ? { ...t, tokens: cloneTokens(draft.tokens) } : t,
    );
    await persist({ themes: next, active_theme_id: draft.sourceId });
    setDraft({ kind: "clean" });
  };

  const saveAsNew = async () => {
    if (draft.kind !== "editing") return;
    const seed = prompt("Name for new theme:", `${active.name} (custom)`);
    if (!seed || !seed.trim()) return;
    const pool = allThemes(config);
    const name = uniqueName(seed.trim(), pool);
    const id = uniqueId(slugify(name), pool);
    const newTheme: Theme = {
      id,
      name,
      builtin: false,
      tokens: cloneTokens(draft.tokens),
    };
    await persist({
      themes: [...config.themes, newTheme],
      active_theme_id: id,
    });
    setDraft({ kind: "clean" });
  };

  const deleteActive = async () => {
    if (active.builtin) return;
    if (!confirm(`Delete theme "${active.name}"?`)) return;
    const next = config.themes.filter((t) => t.id !== active.id);
    await persist({ themes: next, active_theme_id: "dark" });
  };

  const doExport = async () => {
    const path = await saveDialog({
      defaultPath: `${slugify(active.name)}.json`,
      filters: [{ name: "Theme", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      await exportTheme(path, active.id);
    } catch (e) {
      alert(`Export failed: ${e}`);
    }
  };

  const doImport = async () => {
    const selected = await openDialog({
      filters: [{ name: "Theme", extensions: ["json"] }],
      multiple: false,
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const imported = await importTheme(selected);
      const pool = allThemes(config);
      const name = uniqueName(imported.name, pool);
      const id = uniqueId(slugify(name), pool);
      const theme: Theme = { ...imported, id, name, builtin: false };
      await persist({
        themes: [...config.themes, theme],
        active_theme_id: id,
      });
    } catch (e) {
      alert(`Import failed: ${e}`);
    }
  };

  const editing = draft.kind === "editing";
  const canSaveInPlace = editing && !BUILTIN_THEMES.some((b) => b.id === draft.sourceId);

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold flex items-center gap-2">
          <Palette className="h-4 w-4" />
          Appearance
        </h3>
      </div>

      <div className="space-y-1.5">
        <Label className="text-xs">Theme</Label>
        <div className="flex gap-2">
          <Select value={config.active_theme_id} onValueChange={onPickTheme}>
            <SelectTrigger className="h-8 text-xs flex-1">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {allThemes(config).map((t) => (
                <SelectItem key={t.id} value={t.id} className="text-xs">
                  {t.name} {t.builtin ? "(built-in)" : ""}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Tip text="Delete theme" wrapperClass="shrink-0">
            <Button
              variant="outline"
              size="icon"
              className="h-8 w-8"
              disabled={active.builtin || isSystem}
              onClick={deleteActive}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </Tip>
        </div>
      </div>

      {isSystem && (
        <p className="text-[11px] text-t-muted">
          Following Windows theme. Checked on app start and when settings open.
        </p>
      )}

      {!editing ? (
        <div className="flex flex-wrap gap-2">
          <Button
            size="sm"
            variant="outline"
            className="h-7 text-xs"
            disabled={isSystem}
            onClick={beginEdit}
          >
            Edit
          </Button>
          <Button size="sm" variant="outline" className="h-7 text-xs" onClick={doImport}>
            <Upload className="h-3.5 w-3.5 mr-1" />
            Import
          </Button>
          <Button
            size="sm"
            variant="outline"
            className="h-7 text-xs"
            disabled={isSystem}
            onClick={doExport}
          >
            <Download className="h-3.5 w-3.5 mr-1" />
            Export
          </Button>
        </div>
      ) : (
        <>
          <div className="space-y-2 pt-1">
            {THEME_TOKEN_ORDER.map((key) => {
              const value = shownTokens[key];
              const valid = isValidCssColor(value);
              return (
                <div key={key} className="flex items-center gap-2">
                  <Label className="text-xs w-28 shrink-0">
                    {THEME_TOKEN_LABELS[key]}
                  </Label>
                  <div
                    className="h-6 w-6 rounded border border-t-border shrink-0"
                    style={{ background: valid ? value : "transparent" }}
                  />
                  <Input
                    value={value}
                    onChange={(e) => setToken(key, e.target.value)}
                    className={`text-[11px] h-7 font-mono ${
                      valid ? "" : "border-red-500"
                    }`}
                    spellCheck={false}
                  />
                </div>
              );
            })}
          </div>

          <div className="flex flex-wrap gap-2 pt-1">
            <Tip
              text={
                canSaveInPlace
                  ? "Save changes to this theme"
                  : "Built-in themes can't be overwritten — use Save as new"
              }
              align="left"
            >
              <Button
                size="sm"
                className="h-7 text-xs"
                disabled={!canSaveInPlace}
                onClick={saveInPlace}
              >
                <Save className="h-3.5 w-3.5 mr-1" />
                Save
              </Button>
            </Tip>
            <Button
              size="sm"
              variant="outline"
              className="h-7 text-xs"
              onClick={saveAsNew}
            >
              Save as new
            </Button>
            <Button
              size="sm"
              variant="ghost"
              className="h-7 text-xs"
              onClick={cancelEdit}
            >
              Cancel
            </Button>
          </div>
          {!canSaveInPlace && (
            <p className="text-[11px] text-t-muted">
              Editing a built-in theme — use <strong>Save as new</strong> to keep changes.
            </p>
          )}
        </>
      )}
    </section>
  );
}
