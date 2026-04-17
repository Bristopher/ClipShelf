interface TimerDisplayProps {
  remainingSecs: number;
  totalSecs: number;
  running: boolean;
}

export function TimerDisplay({ remainingSecs, running }: TimerDisplayProps) {
  const mins = String(Math.floor(remainingSecs / 60)).padStart(2, "0");
  const secs = String(remainingSecs % 60).padStart(2, "0");

  let colorClass = "text-muted-foreground";
  let animateClass = "";

  if (running) {
    if (remainingSecs <= 5) {
      colorClass = "text-red-500";
      animateClass = "animate-pulse";
    } else if (remainingSecs <= 10) {
      colorClass = "text-amber-400";
    } else {
      colorClass = "text-white";
    }
  }

  return (
    <div className="w-24 border-l border-border flex flex-col items-center justify-center gap-1">
      <span
        className={`text-2xl font-bold font-mono tabular-nums ${colorClass} ${animateClass}`}
      >
        {mins}:{secs}
      </span>
      {running && (
        <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
          Timer
        </span>
      )}
    </div>
  );
}
