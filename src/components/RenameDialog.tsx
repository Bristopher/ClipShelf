import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { EVENTS } from "@/lib/events";
import { renameFile } from "@/lib/commands";
import { errorMessage, toastError } from "@/lib/toast";
import type { FileCreatedEvent } from "@/types";

const ILLEGAL_CHARS = /[<>:"/\\|?*]/;

function splitName(filename: string): { stem: string; ext: string } {
  const dot = filename.lastIndexOf(".");
  if (dot <= 0) return { stem: filename, ext: "" };
  return { stem: filename.slice(0, dot), ext: filename.slice(dot) };
}

/** Mirror of the backend's mover::expand_rename_tokens — preview only; the
 *  raw text is what gets sent (and stored in the MRU). */
function expandTokens(text: string): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  const time = `${pad(now.getHours())}.${pad(now.getMinutes())}`;
  return text.split("{date}").join(date).split("{time}").join(time);
}

interface RenameDialogProps {
  /** Recently used rename texts (backend-maintained), shown as chips. */
  mru: string[];
}

export function RenameDialog({ mru }: RenameDialogProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [currentFilename, setCurrentFilename] = useState("");
  const [text, setText] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  // Ref mirror so the open-listener (mounted once) sees the latest filename.
  const filenameRef = useRef("");

  // Track latest created filename
  useEffect(() => {
    const unlisten = listen<FileCreatedEvent>(EVENTS.FILE_CREATED, (event) => {
      setCurrentFilename(event.payload.filename);
      filenameRef.current = event.payload.filename;
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Open dialog when hotkey-triggered with key=4. The backend APPENDS the
  // text (" - {text}") to the existing name, so the input starts empty and
  // the preview line below shows the resulting filename live. Drag-drops
  // include the filename directly (no file-created event fired for them).
  useEffect(() => {
    const unlisten = listen<{ key: number; filename?: string }>(
      EVENTS.HOTKEY_TRIGGERED,
      (event) => {
        if (event.payload.key === 4) {
          if (event.payload.filename) {
            setCurrentFilename(event.payload.filename);
            filenameRef.current = event.payload.filename;
          }
          setText("");
          setIsOpen(true);
          requestAnimationFrame(() => inputRef.current?.focus());
        }
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const { stem, ext } = splitName(currentFilename);
  const invalidChar = ILLEGAL_CHARS.test(text);
  const canSubmit = text.trim().length > 0 && !invalidChar;
  const preview =
    text.trim() && currentFilename
      ? `${stem} - ${expandTokens(text.trim())}${ext}`
      : "";

  const handleSubmit = () => {
    if (!canSubmit) return;
    renameFile(text.trim()).catch((e) => toastError(errorMessage(e)));
    setIsOpen(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Rename Clip</DialogTitle>
        </DialogHeader>
        {currentFilename && (
          <p className="text-xs text-muted-foreground truncate">
            Current: {currentFilename}
          </p>
        )}
        <Input
          ref={inputRef}
          autoFocus
          placeholder="Text to append (e.g. clutch ace)..."
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          className="text-sm"
          aria-invalid={invalidChar}
        />
        {invalidChar ? (
          <p className="text-[11px] text-red-400">
            {'Name can\'t contain < > : " / \\ | ? *'}
          </p>
        ) : preview ? (
          <p className="text-[11px] text-muted-foreground truncate">
            → {preview}
          </p>
        ) : null}
        <p className="text-[10px] text-muted-foreground">
          Tokens: {"{date}"} → today's date, {"{time}"} → current time
        </p>
        {mru.length > 0 && (
          <div className="flex flex-wrap gap-1">
            {mru.map((t) => (
              <button
                key={t}
                onClick={() => {
                  setText(t);
                  inputRef.current?.focus();
                }}
                title={`Use "${t}"`}
                className="px-1.5 py-0.5 rounded border border-border text-[10px] text-muted-foreground hover:text-foreground hover:bg-hover max-w-40 truncate"
              >
                {t}
              </button>
            ))}
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={() => setIsOpen(false)}>
            Cancel
          </Button>
          <Button size="sm" onClick={handleSubmit} disabled={!canSubmit}>
            Rename
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
