import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Highlight, Note } from "../../types";

interface SelectedText {
  text: string;
  verse: number | null;
}

export function NotesDrawer({
  bookId,
  open,
  onClose,
  chapterIndex,
  getScrollOffset,
  getSelection,
  clearSelection,
  onJump,
}: {
  bookId: number;
  open: boolean;
  onClose: () => void;
  /** The chapter currently on screen — new notes/highlights are attached to it. */
  chapterIndex: number;
  /** Returns the current scroll fraction (0–1) of the reader. */
  getScrollOffset: () => number;
  /** Returns the remembered page selection (text + verse), or null. */
  getSelection: () => SelectedText | null;
  /** Clears the remembered selection (after a highlight is persisted). */
  clearSelection: () => void;
  /** Jump to a saved highlight/note position. */
  onJump: (chapterIndex: number, scrollOffset: number, verse: number | null) => void;
}) {
  const [highlights, setHighlights] = useState<Highlight[]>([]);
  const [notes, setNotes] = useState<Note[]>([]);
  const [noteText, setNoteText] = useState("");
  const [hint, setHint] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const noteRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!open) return;
    setHint(null);
    setNoteText("");
    Promise.all([
      invoke<Highlight[]>("list_highlights", { bookId }).catch(() => []),
      invoke<Note[]>("list_notes", { bookId }).catch(() => []),
    ]).then(([h, n]) => {
      setHighlights(h);
      setNotes(n);
    });
  }, [open, bookId]);

  async function refresh() {
    const [h, n] = await Promise.all([
      invoke<Highlight[]>("list_highlights", { bookId }).catch(() => []),
      invoke<Note[]>("list_notes", { bookId }).catch(() => []),
    ]);
    setHighlights(h);
    setNotes(n);
  }

  async function addHighlight() {
    const sel = getSelection();
    if (!sel || !sel.text) {
      setHint("Select some text on the page first, then add the highlight.");
      return;
    }
    setBusy(true);
    try {
      await invoke("add_highlight", {
        bookId,
        chapterIndex,
        scrollOffset: getScrollOffset(),
        text: sel.text,
        verseNumber: sel.verse,
      });
      setHint(null);
      window.getSelection()?.removeAllRanges();
      clearSelection();
      await refresh();
    } catch (e) {
      setHint(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function addNote() {
    const text = noteText.trim();
    if (!text) return;
    setBusy(true);
    try {
      await invoke("add_note", {
        bookId,
        chapterIndex,
        scrollOffset: getScrollOffset(),
        text,
        verseNumber: getSelection()?.verse ?? null,
      });
      setNoteText("");
      setHint(null);
      await refresh();
    } catch (e) {
      setHint(String(e));
    } finally {
      setBusy(false);
    }
  }

  function jump(item: { chapter_index: number; scroll_offset: number; verse_number: number | null }) {
    onClose();
    onJump(item.chapter_index, item.scroll_offset, item.verse_number);
  }

  if (!open) return null;

  const entries = [
    ...highlights.map((h) => ({ kind: "highlight" as const, id: h.id, chapter_index: h.chapter_index, scroll_offset: h.scroll_offset, text: h.text, verse_number: h.verse_number, created_at: h.created_at })),
    ...notes.map((n) => ({ kind: "note" as const, id: n.id, chapter_index: n.chapter_index, scroll_offset: n.scroll_offset, text: n.text, verse_number: n.verse_number, created_at: n.created_at })),
  ].sort((a, b) => (a.created_at < b.created_at ? 1 : -1));

  return (
    <div className="fixed inset-0 z-40 flex justify-end" onClick={onClose}>
      <div className="absolute inset-0" style={{ background: "rgba(0,0,0,0.25)" }} />

      <aside
        className="relative h-full w-80 flex flex-col themed-surface"
        style={{ background: "var(--bg-elevated)", borderLeft: "1px solid var(--border)" }}
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex items-center justify-between px-4 py-3 border-b" style={{ borderColor: "var(--border)" }}>
          <h2 className="text-sm font-semibold">Notes &amp; Highlights</h2>
          <button onClick={onClose} className="text-lg leading-none opacity-70 hover:opacity-100">×</button>
        </header>

        <div className="flex-1 overflow-y-auto p-3 space-y-2">
          {entries.length === 0 ? (
            <p className="text-sm px-1 py-6 text-center" style={{ color: "var(--text-muted)" }}>
              No notes or highlights yet.
            </p>
          ) : (
            entries.map((e) => (
              <div key={`${e.kind}-${e.id}`} className="group rounded-lg px-3 py-2 themed-surface" style={{ background: "var(--bg)" }}>
                <button onClick={() => jump(e)} className="w-full text-left">
                  <p className="text-xs leading-tight flex items-center gap-1.5" style={{ color: "var(--text-muted)" }}>
                    <span>{e.kind === "highlight" ? "🖊" : "📝"}</span>
                    <span>Chapter {e.chapter_index + 1}</span>
                    {e.verse_number != null && <span>· v{e.verse_number}</span>}
                    <span>· {new Date(e.created_at).toLocaleDateString()}</span>
                  </p>
                  <p className="mt-1 text-sm leading-snug line-clamp-3">{e.text}</p>
                </button>
                <div className="flex justify-end mt-1">
                  <button
                    onClick={() =>
                      invoke(e.kind === "highlight" ? "delete_highlight" : "delete_note", { id: e.id })
                        .then(() => refresh())
                        .catch((err) => setHint(String(err)))
                    }
                    className="opacity-40 group-hover:opacity-100 transition-opacity text-red-500 text-xs"
                    aria-label={`Delete ${e.kind}`}
                  >
                    ✕
                  </button>
                </div>
              </div>
            ))
          )}
        </div>

        <footer className="p-3 border-t space-y-2" style={{ borderColor: "var(--border)" }}>
          <textarea
            ref={noteRef}
            value={noteText}
            onChange={(e) => setNoteText(e.target.value)}
            placeholder="Write a note at the current position…"
            rows={2}
            className="w-full rounded-lg px-3 py-2 text-sm resize-none outline-none border themed-surface"
            style={{ background: "var(--bg)", borderColor: "var(--border)", color: "var(--text)" }}
          />
          {hint && (
            <p className="text-[11px] leading-snug" style={{ color: "var(--text-muted)" }}>{hint}</p>
          )}
          <div className="flex gap-2">
            <button
              onClick={addNote}
              disabled={busy || !noteText.trim()}
              className="flex-1 rounded-lg py-2 text-sm font-medium transition-opacity hover:opacity-80 disabled:opacity-40"
              style={{ background: "var(--accent)", color: "#1c1a17" }}
            >
              + Add note
            </button>
            <button
              onClick={addHighlight}
              disabled={busy}
              className="flex-1 rounded-lg py-2 text-sm font-medium transition-opacity hover:opacity-80 disabled:opacity-40"
              style={{ background: "var(--accent)", color: "#1c1a17" }}
            >
              + Add highlight
            </button>
          </div>
        </footer>
      </aside>
    </div>
  );
}
