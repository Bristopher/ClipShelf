import { useEffect, useRef } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { LogEntry } from "@/types";

function categoryColor(category: string, level: string): string {
  if (level === "error") return "text-red-400";
  if (level === "warning") return "text-red-300";
  if (category === "file-created") return "text-green-400";
  if (category === "file-moved" || category === "file-renamed") return "text-purple-400";
  if (category === "watcher" || category === "obs") return "text-yellow-400";
  return "text-muted-foreground";
}

interface EventLogProps {
  entries: LogEntry[];
}

export function EventLog({ entries }: EventLogProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [entries]);

  return (
    <ScrollArea className="flex-1 px-3 py-2">
      {entries.length === 0 ? (
        <p className="text-sm text-muted-foreground italic pt-4 text-center">
          Waiting for events...
        </p>
      ) : (
        <div className="space-y-0.5">
          {entries.map((entry, i) => (
            <div key={i} className="flex gap-2 text-xs leading-5 font-mono">
              <span className="text-muted-foreground shrink-0">
                {entry.timestamp}
              </span>
              <span className={categoryColor(entry.category, entry.level)}>
                {entry.message}
              </span>
            </div>
          ))}
          <div ref={bottomRef} />
        </div>
      )}
    </ScrollArea>
  );
}
