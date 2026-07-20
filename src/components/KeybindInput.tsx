import { useRef, useState } from "react";
import { X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Tip } from "@/components/ui/tip";
import { cn } from "@/lib/utils";

interface KeybindInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
}

// Keys that count as modifiers. When pressed alone they don't commit —
// we wait for them to be combined with a "real" key.
const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "OS"]);

/**
 * Records the pressed key combo instead of accepting typed text. Click to
 * focus, then press the keys you want bound. Escape cancels. Keybind is
 * formatted as "ctrl+shift+F13" / "alt+v" / "F13", matching the string
 * format already used in the Rust config (see hotkeys.rs).
 */
export function KeybindInput({
  value,
  onChange,
  placeholder = "Click, then press keys...",
  className,
}: KeybindInputProps) {
  const [recording, setRecording] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    e.preventDefault();
    e.stopPropagation();

    // Escape cancels recording without changing the value.
    if (e.key === "Escape") {
      setRecording(false);
      inputRef.current?.blur();
      return;
    }

    // Wait for a non-modifier key.
    if (MODIFIER_KEYS.has(e.key)) return;

    const parts: string[] = [];
    if (e.ctrlKey) parts.push("ctrl");
    if (e.altKey) parts.push("alt");
    if (e.shiftKey) parts.push("shift");
    if (e.metaKey) parts.push("meta");

    // Single characters go lowercase ("V" → "v"); multi-char keys preserve
    // their case so F13, Space, ArrowUp render as the hotkey crate expects.
    const raw = e.key;
    const mainKey = raw.length === 1 ? raw.toLowerCase() : raw;
    parts.push(mainKey);

    onChange(parts.join("+"));
    setRecording(false);
    inputRef.current?.blur();
  };

  const clear = () => {
    onChange("");
    inputRef.current?.focus();
  };

  const display = recording ? "Press keys..." : value;

  return (
    <div className="relative">
      <Input
        ref={inputRef}
        value={display}
        readOnly
        placeholder={placeholder}
        onFocus={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={handleKeyDown}
        className={cn(
          "text-[11px] h-7 font-mono pr-7 cursor-pointer",
          recording && "ring-2 ring-primary/60",
          className,
        )}
      />
      {value && !recording && (
        <Tip text="Clear" wrapperClass="absolute right-1 top-1/2 -translate-y-1/2">
          <button
            type="button"
            onMouseDown={(e) => e.preventDefault()}
            onClick={clear}
            className="p-0.5 rounded text-t-muted hover:text-t-text hover:bg-hover"
          >
            <X className="h-3 w-3" />
          </button>
        </Tip>
      )}
    </div>
  );
}
