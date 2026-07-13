import { revealInExplorer, openFolder } from "@/lib/commands";
import { errorMessage, toastError, toastSuccess } from "@/lib/toast";

export interface ContextMenuState {
  x: number;
  y: number;
  path: string;
}

interface ContextMenuItem {
  label: string;
  action: () => void;
}

interface EntryContextMenuProps {
  menu: ContextMenuState;
  onClose: () => void;
  /** Extra items appended after the standard Reveal/Play/Copy set (e.g. "Edit game…"). */
  extraItems?: ContextMenuItem[];
}

/**
 * Right-click menu for a file-backed row: Reveal in Explorer / Play clip /
 * Copy path / Copy filename, plus optional caller-supplied items. Shared by
 * EventLog and HistoryPanel so the two clip lists behave identically.
 */
export function EntryContextMenu({ menu, onClose, extraItems = [] }: EntryContextMenuProps) {
  const path = menu.path;
  const filename = path.replace(/^.*[\\/]/, "");

  const copy = (text: string, what: string) => {
    navigator.clipboard
      .writeText(text)
      .then(() => toastSuccess(`${what} copied`))
      .catch((e) => toastError(`Copy failed: ${errorMessage(e)}`));
  };

  const items: ContextMenuItem[] = [
    { label: "Reveal in Explorer", action: () => revealInExplorer(path).catch((e) => toastError(errorMessage(e))) },
    { label: "Play clip", action: () => openFolder(path).catch((e) => toastError(errorMessage(e))) },
    { label: "Copy path", action: () => copy(path, "Path") },
    { label: "Copy filename", action: () => copy(filename, "Filename") },
    ...extraItems,
  ];

  // Keep the menu on-screen near the bottom edge.
  const style: React.CSSProperties = {
    left: Math.min(menu.x, window.innerWidth - 180),
    top: Math.min(menu.y, window.innerHeight - items.length * 28 - 12),
  };

  return (
    <div
      style={style}
      className="fixed z-50 w-44 rounded-md border border-t-border bg-panel shadow-lg py-1 animate-in fade-in-0 zoom-in-95 duration-100"
      // mousedown-outside closes the menu — swallow it inside so item
      // clicks still land.
      onMouseDown={(e) => e.stopPropagation()}
    >
      {items.map((item) => (
        <button
          key={item.label}
          onClick={() => {
            item.action();
            onClose();
          }}
          className="block w-full text-left px-3 py-1 text-xs text-t-text hover:bg-hover"
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
