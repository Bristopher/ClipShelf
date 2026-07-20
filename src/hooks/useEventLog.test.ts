import { expect, test } from "vitest";
import { appendLogEntry } from "./useEventLog";
import type { LogEntry } from "@/types";

const entry = (message: string, category = "watcher-status"): LogEntry => ({
  timestamp: "01:00:00 PM",
  level: "info",
  message,
  category,
});

test("repeated watcher-status message replaces the old line", () => {
  const a = entry("Watcher running");
  const b = { ...entry("Watcher running"), timestamp: "02:00:00 PM" };
  const out = appendLogEntry([a], b);
  expect(out).toHaveLength(1);
  expect(out[0].timestamp).toBe("02:00:00 PM");
});

test("different watcher-status messages both stay", () => {
  const out = appendLogEntry([entry("Watcher running")], entry("Watcher stopped"));
  expect(out.map((e) => e.message)).toEqual(["Watcher running", "Watcher stopped"]);
});

test("non-watcher entries never dedupe and keep order", () => {
  const moved = entry("Moved: clip.mp4", "file-moved");
  const out = appendLogEntry([moved], { ...moved });
  expect(out).toHaveLength(2);
});

test("replacement moves the line to the bottom past other entries", () => {
  const out = appendLogEntry(
    [entry("Watcher running"), entry("Moved: a.mp4", "file-moved")],
    { ...entry("Watcher running"), timestamp: "03:00:00 PM" },
  );
  expect(out.map((e) => e.message)).toEqual(["Moved: a.mp4", "Watcher running"]);
});

test("cap trims oldest entries", () => {
  const many = Array.from({ length: 5 }, (_, i) => entry(`m${i}`, "system"));
  const out = appendLogEntry(many, entry("new", "system"), 3);
  expect(out.map((e) => e.message)).toEqual(["m3", "m4", "new"]);
});
