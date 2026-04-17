import { useEffect, useState } from "react";
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
import type { FileCreatedEvent } from "@/types";

export function RenameDialog() {
  const [isOpen, setIsOpen] = useState(false);
  const [currentFilename, setCurrentFilename] = useState("");
  const [text, setText] = useState("");

  // Track latest created filename
  useEffect(() => {
    const unlisten = listen<FileCreatedEvent>(EVENTS.FILE_CREATED, (event) => {
      setCurrentFilename(event.payload.filename);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Open dialog when hotkey-triggered with key=4
  useEffect(() => {
    const unlisten = listen<{ key: number }>(EVENTS.HOTKEY_TRIGGERED, (event) => {
      if (event.payload.key === 4) {
        setText("");
        setIsOpen(true);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleSubmit = () => {
    if (text.trim()) {
      renameFile(text.trim());
    }
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
          autoFocus
          placeholder="Enter new name..."
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          className="text-sm"
        />
        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={() => setIsOpen(false)}>
            Cancel
          </Button>
          <Button size="sm" onClick={handleSubmit}>
            Rename
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
