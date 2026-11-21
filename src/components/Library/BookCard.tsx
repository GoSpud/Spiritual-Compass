import { useEffect, useRef, useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { BookWithProgress, Category } from "../../types";
import { CATEGORIES } from "../../types";

export function BookCard({ book, onOpen, onDeleted }: { book: BookWithProgress; onOpen: () => void; onDeleted?: () => void }) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close the card menu when clicking elsewhere.
  useEffect(() => {
    if (!menuOpen) return;
    function onClick(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenuOpen(false);
    }
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, [menuOpen]);

  async function changeCategory(cat: Category) {
    await invoke("update_book_category", { id: book.id, category: cat });
    setMenuOpen(false);
  }

  async function remove() {
    if (!confirm(`Remove "${book.title}" from your library?`)) return;
    await invoke("delete_book", { id: book.id });
    onDeleted?.(); // ask the parent to refresh its list
  }

  // convertFileSrc turns a local path into an asset-protocol URL the webview can load.
  const coverSrc = book.cover_path ? convertFileSrc(book.cover_path) : null;

  return (
    <div className="group relative">
      <button
        onClick={onOpen}
        className="w-full text-left rounded-xl overflow-hidden themed-surface transition-transform group-hover:-translate-y-0.5"
        style={{ background: "var(--surface)", boxShadow: "var(--shadow)" }}
      >
        {/* Cover */}
        <div className="aspect-[2/3] w-full bg-black/10 flex items-center justify-center overflow-hidden">
          {coverSrc ? (
            <img src={coverSrc} alt="" className="h-full w-full object-cover" />
          ) : (
            <span className="text-4xl opacity-50">📕</span>
          )}
        </div>

        {/* Meta */}
        <div className="p-2.5 space-y-1">
          <p className="text-sm font-medium leading-tight line-clamp-2">{book.title}</p>
          {book.author && (
            <p className="text-xs truncate" style={{ color: "var(--text-muted)" }}>{book.author}</p>
          )}
          <span
            className="inline-block text-[10px] px-1.5 py-0.5 rounded font-medium"
            style={{ background: "var(--accent-soft)", color: "var(--accent)" }}
          >
            {book.category}
          </span>

          {/* Progress */}
          <div className="h-1 w-full rounded-full overflow-hidden mt-1.5" style={{ background: "var(--border)" }}>
            <div
              className="h-full rounded-full transition-all"
              style={{ width: `${Math.round(book.percent ?? 0)}%`, background: "var(--accent)" }}
            />
          </div>
        </div>
      </button>

      {/* Card menu */}
      <div ref={menuRef} className="absolute top-2 right-2">
        <button
          onClick={(e) => { e.stopPropagation(); setMenuOpen((v) => !v); }}
          className="w-7 h-7 rounded-lg flex items-center justify-center text-sm themed-surface"
          style={{ background: "var(--bg-elevated)", border: "1px solid var(--border)" }}
          aria-label="Book options"
        >
          ⋯
        </button>

        {menuOpen && (
          <div
            className="absolute right-0 mt-1 w-44 rounded-lg py-1 themed-surface z-20"
            style={{ background: "var(--bg-elevated)", border: "1px solid var(--border)", boxShadow: "var(--shadow)" }}
          >
            <p className="px-3 py-1 text-[11px] uppercase tracking-wide" style={{ color: "var(--text-muted)" }}>
              Move to…
            </p>
            {CATEGORIES.map((c) => (
              <button
                key={c}
                onClick={() => changeCategory(c)}
                className="w-full text-left px-3 py-1.5 text-sm hover:bg-black/5 dark:hover:bg-white/10"
              >
                {c}
              </button>
            ))}
            <div className="h-px my-1" style={{ background: "var(--border)" }} />
            <button onClick={remove} className="w-full text-left px-3 py-1.5 text-sm text-red-500 hover:bg-black/5 dark:hover:bg-white/10">
              Remove from library
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
