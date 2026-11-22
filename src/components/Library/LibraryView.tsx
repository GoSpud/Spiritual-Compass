import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { BookWithProgress, Category, SearchHit } from "../../types";
import { CATEGORIES } from "../../types";
import { BookCard } from "./BookCard";
import { ImportDialog } from "./ImportDialog";
import { useTheme } from "../../theme/ThemeProvider";

type Filter = "All Books" | Category;

const FILTERS: Filter[] = ["All Books", ...CATEGORIES];

export function LibraryView({ onOpenBook, onOpenAt }: {
  onOpenBook: (b: BookWithProgress) => void;
  onOpenAt?: (b: BookWithProgress, chapterIndex: number) => void;
}) {
  const { theme, setTheme } = useTheme();
  const [books, setBooks] = useState<BookWithProgress[]>([]);
  const [filter, setFilter] = useState<Filter>("All Books");
  const [search, setSearch] = useState("");
  const [importing, setImporting] = useState(false);

  // Import flow: after picking files we ask which category they go into first.
  const [pendingPaths, setPendingPaths] = useState<string[] | null>(null);
  const [pickCategory, setPickCategory] = useState<Category>("EGW Library");

  // Full-text search results (null = not currently searching).
  const [hits, setHits] = useState<SearchHit[] | null>(null);

  const isSearching = search.trim().length > 0;

  const load = useCallback(async () => {
    try {
      const cat = filter === "All Books" ? undefined : filter;
      const rows: BookWithProgress[] = await invoke("list_books", { category: cat, search: null });
      setBooks(rows);
    } catch (e) {
      console.error("failed to list books", e);
    }
  }, [filter]);

  useEffect(() => { load(); }, [load]);

  // Run full-text search as the user types (debounced).
  useEffect(() => {
    if (!isSearching) { setHits(null); return; }
    const q = search.trim();
    let cancelled = false;
    const t = setTimeout(async () => {
      try {
        const res: SearchHit[] = await invoke("search_books", { query: q });
        if (!cancelled) setHits(res);
      } catch (e) {
        console.error("search failed", e);
        if (!cancelled) setHits([]);
      }
    }, 180);
    return () => { cancelled = true; clearTimeout(t); };
  }, [search, isSearching]);

  const visible = useMemo(
    () => books.filter((b) => filter === "All Books" || b.category === filter),
    [books, filter],
  );

  async function pickFiles() {
    const selected: string | string[] | null = await open({
      multiple: true,
      filters: [{ name: "EPUB", extensions: ["epub"] }],
    });
    if (!selected) return;
    setPendingPaths(Array.isArray(selected) ? selected : [selected]);
  }

  async function confirmImport() {
    if (!pendingPaths) return;
    try {
      setImporting(true);
      await invoke("import_epubs", { paths: pendingPaths, category: pickCategory });
      await load();
      // Jump to the chosen category so the user sees their books land there.
      setFilter(pickCategory);
    } catch (e) {
      console.error("import failed", e);
    } finally {
      setImporting(false);
      setPendingPaths(null);
    }
  }

  function cancelImport() { setPendingPaths(null); }

  return (
    <div className="flex h-full">
      {/* Sidebar */}
      <aside className="w-64 shrink-0 border-r themed-surface flex flex-col" style={{ borderColor: "var(--border)" }}>
        <div className="p-4 space-y-1">
          {FILTERS.map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
                filter === f ? "font-medium" : "hover:bg-black/5 dark:hover:bg-white/10"
              }`}
              style={filter === f ? { background: "var(--accent-soft)", color: "var(--accent)" } : undefined}
            >
              {f}
            </button>
          ))}
        </div>

        <div className="px-4 pb-6">
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search books…"
            className="w-full rounded-lg px-3 py-2 text-sm outline-none border themed-surface"
            style={{ borderColor: "var(--border)", background: "var(--bg-elevated)" }}
          />
        </div>

        <footer className="mt-auto px-4 pb-4 pt-6 shrink-0">
          <button
            onClick={() => { const themes = ["light", "dark", "reading"] as const; setTheme(themes[(themes.indexOf(theme) + 1) % 3]!); }}
            className="rounded-lg px-3 py-1.5 text-xs font-medium transition-opacity hover:opacity-80"
            style={{ background: "var(--accent-soft)", color: "var(--accent)" }}
            title="Cycle theme (Light / Dark / Reading)"
          >
            {theme === "light" ? "☀ Light" : theme === "dark" ? "🌙 Dark" : "📜 Reading"}
          </button>
        </footer>
      </aside>

      {/* Main */}
      <main className="flex-1 overflow-y-auto">
        <header className="sticky top-0 z-10 flex items-center justify-between px-6 py-4 themed-surface" style={{ background: "var(--bg)" }}>
          <h1 className="text-lg font-semibold">{filter}</h1>
          <button
            onClick={pickFiles}
            disabled={importing || pendingPaths !== null}
            className="rounded-lg px-4 py-2 text-sm font-medium transition-opacity disabled:opacity-50"
            style={{ background: "var(--accent)", color: "#1c1a17" }}
          >
            {importing ? "Importing…" : "Import EPUB"}
          </button>
        </header>

        <div className="px-6 pb-2">
          {isSearching ? (
            hits === null ? (
              <p className="text-sm py-10 text-center" style={{ color: "var(--text-muted)" }}>Searching…</p>
            ) : hits.length === 0 ? (
              <EmptyState filter={filter} onImport={pickFiles} noResults={search.trim()} />
            ) : (
              <SearchResults hits={hits} books={books} onOpenAt={(b, ch) => onOpenAt?.(b, ch)} onOpenBook={onOpenBook} />
            )
          ) : visible.length === 0 ? (
            <EmptyState filter={filter} onImport={pickFiles} />
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-4">
              {visible.map((b) => (
                <BookCard key={b.id} book={b} onOpen={() => onOpenBook(b)} onDeleted={load} />
              ))}
            </div>
          )}
        </div>
      </main>

      {pendingPaths !== null ? (
        <CategoryPicker
          count={pendingPaths.length}
          category={pickCategory}
          setCategory={setPickCategory}
          onConfirm={confirmImport}
          onCancel={cancelImport}
        />
      ) : importing && <ImportDialog />}
    </div>
  );
}

function CategoryPicker({ count, category, setCategory, onConfirm, onCancel }: {
  count: number;
  category: Category;
  setCategory: (c: Category) => void;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.35)" }}>
      <div
        className="rounded-xl px-6 py-5 themed-surface w-[24rem]"
        style={{ background: "var(--bg-elevated)", border: "1px solid var(--border)", boxShadow: "var(--shadow)" }}
      >
        <p className="text-sm font-medium mb-3">
          {count === 1 ? "Where should this book go?" : `Where should these ${count} books go?`}
        </p>
        <div className="space-y-1.5 mb-4">
          {CATEGORIES.map((c) => (
            <button
              key={c}
              onClick={() => setCategory(c)}
              className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
                category === c ? "font-medium" : "hover:bg-black/5 dark:hover:bg-white/10"
              }`}
              style={category === c ? { background: "var(--accent-soft)", color: "var(--accent)" } : undefined}
            >
              {c}
            </button>
          ))}
        </div>
        <div className="flex gap-2 justify-end">
          <button onClick={onCancel} className="rounded-lg px-3 py-1.5 text-sm" style={{ background: "var(--bg)" }}>
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className="rounded-lg px-4 py-1.5 text-sm font-medium"
            style={{ background: "var(--accent)", color: "#1c1a17" }}
          >
            Import into {category}
          </button>
        </div>
      </div>
    </div>
  );
}

function EmptyState({ filter, onImport, noResults }: { filter: Filter; onImport: () => void; noResults?: string }) {
  const label = filter === "All Books" ? "books" : filter.toLowerCase();
  if (noResults) {
    return (
      <div className="flex flex-col items-center justify-center py-24 text-center">
        <div className="mb-4 text-5xl opacity-60">🔎</div>
        <p className="text-base mb-1">No matches for “{noResults}”.</p>
        <p className="text-sm" style={{ color: "var(--text-muted)" }}>
          We searched titles, authors, and the full text of every book.
        </p>
      </div>
    );
  }
  return (
    <div className="flex flex-col items-center justify-center py-24 text-center">
      <div className="mb-4 text-5xl opacity-60">📖</div>
      <p className="text-base mb-1">No {label} yet.</p>
      <p className="text-sm mb-6" style={{ color: "var(--text-muted)" }}>
        Import an .epub to get started.
      </p>
      <button
        onClick={onImport}
        className="rounded-lg px-4 py-2 text-sm font-medium"
        style={{ background: "var(--accent)", color: "#1c1a17" }}
      >
        Import EPUB
      </button>
    </div>
  );
}

function SearchResults({ hits, books, onOpenAt, onOpenBook }: {
  hits: SearchHit[];
  books: BookWithProgress[];
  onOpenAt?: (b: BookWithProgress, chapterIndex: number) => void;
  onOpenBook: (b: BookWithProgress) => void;
}) {
  // Group hits by book so a single book with several match types shows once.
  const byBook = new Map<number, SearchHit[]>();
  for (const h of hits) {
    if (!byBook.has(h.book_id)) byBook.set(h.book_id, []);
    byBook.get(h.book_id)!.push(h);
  }

  return (
    <div className="space-y-3">
      {[...byBook.entries()].map(([bookId, bookHits]) => {
        const book = books.find((b) => b.id === bookId);
        if (!book) return null;
        // Prefer a text hit so we can jump to the matching chapter.
        const textHit = bookHits.find((h) => h.matchType === "text");
        const first = bookHits[0];
        return (
          <button
            key={bookId}
            onClick={() => onOpenAt && textHit ? onOpenAt(book, textHit.chapterIndex) : onOpenBook(book)}
            className="w-full text-left rounded-xl p-4 themed-surface transition-colors hover:bg-black/5 dark:hover:bg-white/10"
            style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
          >
            <div className="flex items-center justify-between gap-3">
              <p className="text-sm font-medium truncate">{book.title}</p>
              <span
                className="shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium"
                style={{ background: "var(--accent-soft)", color: "var(--accent)" }}
              >
                {first.matchType === "text" ? "in text" : first.matchType}
              </span>
            </div>
            <p className="mt-1.5 text-sm leading-relaxed line-clamp-3" style={{ color: "var(--text-muted)" }}
               dangerouslySetInnerHTML={{ __html: first.snippet }} />
          </button>
        );
      })}
    </div>
  );
}
