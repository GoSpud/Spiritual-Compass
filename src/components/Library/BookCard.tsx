import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { BookWithProgress, Category } from "../../types";
import { CATEGORIES, categoryLabel } from "../../types";

// Deterministic pseudo-random tilt so each book sits slightly differently on
// the shelf (a small lay-back fan, no layout churn).
function tiltFor(id: number): number {
  const x = Math.sin(id * 12.9898) * 43758.5453;
  const frac = x - Math.floor(x);
  return (frac - 0.5) * 2.4; // ~ -1.2deg .. +1.2deg
}

export function BookCard({ book, onOpen, onDeleted }: { book: BookWithProgress; onOpen: () => void; onDeleted?: () => void }) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close the menu when clicking anywhere else.
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
  const tilt = tiltFor(book.id);
  const pct = Math.round(book.percent ?? 0);

  return (
    <div className="shelf-slot">
      <button
        type="button"
        onClick={onOpen}
        className="shelf-book"
        style={{ "--tilt": `${tilt}deg` } as CSSProperties}
        title={book.title}
        aria-label={`Open ${book.title}`}
      >
        <div className="shelf-cover">
          {coverSrc ? (
            <img src={coverSrc} alt="" className="h-full w-full object-cover" />
          ) : (
            <span className="shelf-fallback" aria-hidden="true">📕</span>
          )}
          <span className="shelf-spine" aria-hidden="true" />
          <span className="shelf-progress" aria-hidden="true">
            <span style={{ height: `${pct}%` }} />
          </span>
          {pct > 0 && <span className="shelf-pct">{pct}%</span>}
        </div>
      </button>

      {/* ⋯ menu — sibling of the cover so it stays stable while the book tilts */}
      <div ref={menuRef} className={`shelf-menu ${menuOpen ? "is-open" : ""}`}>
        <button
          type="button"
          onClick={(e) => { e.stopPropagation(); setMenuOpen((v) => !v); }}
          className="shelf-menu-btn"
          aria-label={`Options for ${book.title}`}
          aria-expanded={menuOpen}
        >
          ⋯
        </button>

        {menuOpen && (
          <div className="shelf-menu-popover">
            <p className="px-3 py-1 text-[11px] uppercase tracking-wide" style={{ color: "var(--text-muted)" }}>
              Move to…
            </p>
            {CATEGORIES.map((c) => (
              <button
                key={c}
                type="button"
                onClick={() => changeCategory(c)}
                className="w-full text-left px-3 py-1.5 text-sm hover:bg-black/5 dark:hover:bg-white/10"
              >
                {categoryLabel(c)}
              </button>
            ))}
            <div className="h-px my-1" style={{ background: "var(--border)" }} />
            <button
              type="button"
              onClick={remove}
              className="w-full text-left px-3 py-1.5 text-sm text-red-500 hover:bg-black/5 dark:hover:bg-white/10"
            >
              Remove from library
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
