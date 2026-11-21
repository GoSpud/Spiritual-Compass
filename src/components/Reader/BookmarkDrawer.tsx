import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface BookmarkRow {
  id: number;
  book_id: number;
  chapter_index: number;
  scroll_offset: number;
  created_at: string;
}

export function BookmarkDrawer({
  bookId,
  open,
  onClose,
  onJump,
  onAdd,
}: {
  bookId: number;
  open: boolean;
  onClose: () => void;
  onJump: (chapterIndex: number, scrollOffset: number) => void;
  /** Called to persist a bookmark at the reader's current position. */
  onAdd: () => Promise<void>;
}) {
  const [items, setItems] = useState<BookmarkRow[]>([]);

  useEffect(() => {
    if (!open) return;
    invoke<BookmarkRow[]>("list_bookmarks", { bookId })
      .then(setItems)
      .catch((e) => console.error("failed to list bookmarks", e));
  }, [open, bookId]);

  async function add() {
    await onAdd();
    const rows: BookmarkRow[] = await invoke("list_bookmarks", { bookId });
    setItems(rows);
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-40 flex justify-end" onClick={onClose}>
      {/* Backdrop */}
      <div className="absolute inset-0" style={{ background: "rgba(0,0,0,0.25)" }} />

      {/* Panel */}
      <aside
        className="relative h-full w-80 flex flex-col themed-surface"
        style={{ background: "var(--bg-elevated)", borderLeft: "1px solid var(--border)" }}
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex items-center justify-between px-4 py-3 border-b" style={{ borderColor: "var(--border)" }}>
          <h2 className="text-sm font-semibold">Bookmarks</h2>
          <button onClick={onClose} className="text-lg leading-none opacity-70 hover:opacity-100">×</button>
        </header>

        <div className="flex-1 overflow-y-auto p-3 space-y-2">
          {items.length === 0 ? (
            <p className="text-sm px-1 py-4 text-center" style={{ color: "var(--text-muted)" }}>
              No bookmarks yet.
            </p>
          ) : (
            items.map((b) => (
              <div key={b.id} className="group flex items-center gap-2 rounded-lg px-3 py-2 themed-surface" style={{ background: "var(--bg)" }}>
                <button onClick={() => onJump(b.chapter_index, b.scroll_offset)} className="flex-1 text-left">
                  <p className="text-sm leading-tight">Chapter {b.chapter_index + 1}</p>
                  <p className="text-[11px]" style={{ color: "var(--text-muted)" }}>
                    {new Date(b.created_at).toLocaleString()}
                  </p>
                </button>
                <button
                  onClick={() => invoke("delete_bookmark", { id: b.id }).then(() => setItems((xs) => xs.filter((x) => x.id !== b.id)))}
                  className="opacity-40 group-hover:opacity-100 transition-opacity text-red-500"
                  aria-label="Delete bookmark"
                >
                  ✕
                </button>
              </div>
            ))
          )}
        </div>

        <footer className="p-3 border-t" style={{ borderColor: "var(--border)" }}>
          <button
            onClick={add}
            className="w-full rounded-lg py-2 text-sm font-medium transition-opacity hover:opacity-80"
            style={{ background: "var(--accent)", color: "#1c1a17" }}
          >
            + Add bookmark at current position
          </button>
        </footer>
      </aside>
    </div>
  );
}
