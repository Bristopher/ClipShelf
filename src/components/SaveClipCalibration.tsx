import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { startCalibration, cancelCalibration, updateConfig } from "@/lib/commands";
import type { CalibrationSampleEvent, AppConfig } from "@/types";

interface Props {
  config: AppConfig;
  onConfigChange: (c: AppConfig) => void;
}

interface Row {
  filename: string;
  deltaMs: number;
  index: number;
}

export function SaveClipCalibration({ config, onConfigChange }: Props) {
  const [target, setTarget] = useState(5);
  const [running, setRunning] = useState(false);
  const [rows, setRows] = useState<Row[]>([]);
  const [summary, setSummary] = useState<{
    averageMs: number;
    worstMs: number;
    bestMs: number;
  } | null>(null);

  useEffect(() => {
    const unlisten = listen<CalibrationSampleEvent>("calibration-event", (e) => {
      const p = e.payload;
      setRows((prev) => [
        ...prev,
        { filename: p.filename, deltaMs: p.deltaMs, index: p.index },
      ]);
      if (p.kind === "complete") {
        setRunning(false);
        if (p.averageMs != null && p.worstMs != null && p.bestMs != null) {
          setSummary({
            averageMs: p.averageMs,
            worstMs: p.worstMs,
            bestMs: p.bestMs,
          });
        }
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const start = async () => {
    setRows([]);
    setSummary(null);
    try {
      await startCalibration(target);
      setRunning(true);
    } catch (err) {
      alert(String(err));
    }
  };

  const cancel = async () => {
    await cancelCalibration();
    setRunning(false);
  };

  const applySuggested = async () => {
    if (!summary) return;
    // Pick worst + 50% headroom, rounded up, min 2s.
    const suggested = Math.max(2, Math.ceil((summary.worstMs * 1.5) / 1000));
    const updated = await updateConfig({
      save_clip_health_check_timeout_secs: suggested,
    });
    onConfigChange(updated);
  };

  const fmt = (ms: number) => `${(ms / 1000).toFixed(2)}s`;

  return (
    <div className="space-y-2 p-2 rounded border border-t-border bg-panel/40">
      <div className="flex items-center justify-between gap-2">
        <Label className="text-xs">Test save-clip latency</Label>
        {!running ? (
          <div className="flex items-center gap-1">
            <Input
              type="number"
              min={1}
              max={20}
              value={target}
              onChange={(e) => setTarget(Number(e.target.value) || 5)}
              className="text-xs h-7 w-14"
            />
            <Button
              size="sm"
              variant="outline"
              className="h-7 text-xs"
              onClick={start}
              disabled={!config.save_clip_bind}
            >
              Start
            </Button>
          </div>
        ) : (
          <Button
            size="sm"
            variant="outline"
            className="h-7 text-xs"
            onClick={cancel}
          >
            Cancel
          </Button>
        )}
      </div>

      <p className="text-[10px] text-t-muted">
        Click Start, then press your save-clip key {target} times during normal
        gameplay. We'll measure how long each clip takes to land on disk so you
        can pick a sensible timeout.
      </p>

      {(running || rows.length > 0) && (
        <div className="space-y-1">
          <p className="text-[10px] text-t-muted">
            {running
              ? `Waiting for press ${rows.length + 1} / ${target}...`
              : `Done. ${rows.length} samples.`}
          </p>
          <ul className="space-y-0.5 max-h-32 overflow-auto">
            {rows.map((r) => (
              <li
                key={r.index}
                className="flex justify-between text-[10px] font-mono"
              >
                <span className="truncate pr-2">
                  #{r.index} {r.filename}
                </span>
                <span className="shrink-0">{fmt(r.deltaMs)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {summary && (
        <div className="space-y-1 pt-1 border-t border-t-border">
          <div className="flex justify-between text-[10px]">
            <span className="text-t-muted">Best / Avg / Worst</span>
            <span className="font-mono">
              {fmt(summary.bestMs)} / {fmt(summary.averageMs)} /{" "}
              {fmt(summary.worstMs)}
            </span>
          </div>
          <Button
            size="sm"
            variant="outline"
            className="h-7 text-xs w-full"
            onClick={applySuggested}
          >
            Use suggested ({Math.max(2, Math.ceil((summary.worstMs * 1.5) / 1000))}s
            — worst + 50% headroom)
          </Button>
        </div>
      )}
    </div>
  );
}
