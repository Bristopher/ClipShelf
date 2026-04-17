import { Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { pressGkey } from "@/lib/commands";

const gkeys = [
  { key: 1, label: "G1", tag: "!!", color: "bg-blue-600 hover:bg-blue-700" },
  { key: 2, label: "G2", tag: "CHKD", color: "bg-green-600 hover:bg-green-700" },
  { key: 3, label: "G3", tag: "!!!", color: "bg-orange-600 hover:bg-orange-700" },
  { key: 4, label: "G4", tag: "REN", color: "bg-purple-600 hover:bg-purple-700" },
] as const;

interface SidebarProps {
  onSettingsClick: () => void;
}

export function Sidebar({ onSettingsClick }: SidebarProps) {
  return (
    <aside className="w-16 border-r border-border flex flex-col gap-1 p-1.5">
      {gkeys.map((g) => (
        <button
          key={g.key}
          onClick={() => pressGkey(g.key)}
          className={`${g.color} rounded px-1.5 py-1.5 text-white text-[10px] font-bold flex flex-col items-center gap-0 transition-transform active:scale-95`}
        >
          <span>{g.label}</span>
          <span className="text-[8px] opacity-80 leading-tight">{g.tag}</span>
        </button>
      ))}
      <div className="flex-1" />
      <Button
        variant="ghost"
        size="icon"
        className="mx-auto h-7 w-7"
        onClick={onSettingsClick}
      >
        <Settings className="h-4 w-4" />
      </Button>
    </aside>
  );
}
