import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { BookWithProgress, ReadingProgress } from "../../types";
import { useTheme } from "../../theme/ThemeProvider";
import { TEXT_COLOR_ORDER } from "../../theme/ThemeProvider";
import type { ThemeName } from "../../theme/ThemeProvider";
import { BookmarkDrawer } from "./BookmarkDrawer";
import { GoToDrawer } from "./GoToDrawer";
import type { BibleTocBook } from "./GoToDrawer";

interface ChapterInfo {
  index: number;
  title: string;
  href: string;
}

const FONT_SIZES = [14, 16, 18, 20, 22, 24];
const LINE_HEIGHTS = [1.5, 1.7, 1.9, 2.1];

export function ReaderView({ book, initialChapter, onBack }: {
  book: BookWithProgress;
  initialChapter?: number | null;
  onBack: () => void;
}) {
  const { theme, setTheme, textColor, setTextColor } = useTheme();

  const [chapters, setChapters] = useState<ChapterInfo[]>([]);
  const [chapterIndex, setChapterIndex] = useState(0);
  const [html, setHtml] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const cycleTextColor = () => {
    const idx = TEXT_COLOR_ORDER.indexOf(textColor);
    setTextColor(TEXT_COLOR_ORDER[(idx + 1) % TEXT_COLOR_ORDER.length]);
  };

  // Reader settings (persisted via settings table).
  const [fontSize, setFontSize] = useState(18);
  const [lineHeight, setLineHeight] = useState(1.7);

  const [bookmarksOpen, setBookmarksOpen] = useState(false);
  const [navOpen, setNavOpen] = useState(false);
  const [toc, setToc] = useState<BibleTocBook[] | null>(null);
  // Verse to scroll to once the current chapter HTML is in the DOM.
  const [pendingVerse, setPendingVerse] = useState<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const restoreDoneRef = useRef(false);
  // Saved position from the mount effect, applied by the restore effect once
  // the page is rendered — avoids a second get_progress round-trip.
  const pendingRestoreRef = useRef<{ chapterIndex: number; scrollOffset: number } | null>(null);

  // Drag-to-flip: a horizontal swipe turns the page (like turning a book page).
  const dragStart = useRef<{ x: number; y: number } | null>(null);
  const [dragging, setDragging] = useState(false);

  // Cache of loaded chapter HTML so a turn never waits on the backend.
  const cacheRef = useRef(new Map<number, string>());

  // Slide transition state. While a turn is in flight we render an overlay
  // "track" holding both pages side by side and slide it horizontally — one
  // rigid motion with no vertical component, like an e-reader page slide.
  const [slide, setSlide] = useState<{ dir: "next" | "prev"; outHtml: string; inHtml: string; off: number; inIndex: number } | null>(null);
  const turnBusyRef = useRef(false);

  // Load chapters + restore last position on mount.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [ch, progress] = await Promise.all([
          invoke<ChapterInfo[]>("get_chapters", { bookId: book.id }),
          invoke<ReadingProgress | null>("get_progress", { bookId: book.id }).catch(() => null),
        ]);
        if (cancelled) return;
        setChapters(ch);

        // Bibles get the Book/Chapter/Verse navigator.
        if (book.category === "Bibles") {
          invoke<BibleTocBook[]>("get_bible_toc", { bookId: book.id })
            .then((t) => { if (!cancelled && t.length > 0) setToc(t); })
            .catch(() => {});
        }

        // A search result can ask us to open at a specific chapter.
        const start = initialChapter != null
          ? Math.min(initialChapter, ch.length - 1)
          : progress ? Math.min(progress.chapter_index, ch.length - 1) : 0;
        // Remember the saved position so the restore effect below can scroll to it.
        pendingRestoreRef.current = initialChapter != null || !progress
          ? null
          : { chapterIndex: start, scrollOffset: progress.scroll_offset };
        setChapterIndex(start);
      } catch (e) {
        console.error(e);
        setError(String(e));
        setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [book.id]);

  // Mouse back button (button 4) returns to the library/menu.
  useEffect(() => {
    function onMouseDown(e: MouseEvent) {
      if (e.button === 3) leave(); // browser/mouse "back" button
    }
    window.addEventListener("mousedown", onMouseDown);
    return () => window.removeEventListener("mousedown", onMouseDown);
  }, [onBack]);

  // Load chapter HTML whenever the index changes (served from cache when possible).
  useEffect(() => {
    if (chapters.length === 0) return;
    let cancelled = false;
    const cached = cacheRef.current.get(chapterIndex);
    if (cached != null) {
      setHtml(cached);
      setLoading(false);
      preloadNeighbors(chapterIndex);
      return;
    }
    setLoading(true);
    invoke<string>("get_chapter_html", { bookId: book.id, chapterIndex })
      .then((h) => {
        if (cancelled) return;
        cacheRef.current.set(chapterIndex, h);
        setHtml(h);
        preloadNeighbors(chapterIndex);
      })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [book.id, chapterIndex, chapters.length]);

  // Keep adjacent pages warm so a turn never has to wait on the backend.
  function preloadNeighbors(i: number) {
    for (const j of [i - 1, i + 1]) {
      if (j >= 0 && j < chapters.length && !cacheRef.current.has(j)) {
        invoke<string>("get_chapter_html", { bookId: book.id, chapterIndex: j })
          .then((h) => { cacheRef.current.set(j, h); })
          .catch(() => {});
      }
    }
  }

  // Restore the saved scroll offset after the first chapter renders.
  useEffect(() => {
    if (restoreDoneRef.current || loading) return;
    const restore = pendingRestoreRef.current;
    if (!restore) return;
    if (restore.chapterIndex !== chapterIndex) return;
    const el = scrollRef.current;
    if (!el) return;
    restoreDoneRef.current = true;
    const max = Math.max(0, el.scrollHeight - el.clientHeight);
    el.scrollTop = restore.scrollOffset * max;
  }, [loading, chapterIndex, book.id]);

  // Persist reader settings.
  useEffect(() => { invoke("set_setting", { key: "fontSize", value: String(fontSize) }).catch(() => {}); }, [fontSize]);
  useEffect(() => { invoke("set_setting", { key: "lineHeight", value: String(lineHeight) }).catch(() => {}); }, [lineHeight]);

  // Load persisted reader settings.
  useEffect(() => {
    invoke<string>("get_setting", { key: "fontSize" }).then((v) => v && setFontSize(Number(v))).catch(() => {});
    invoke<string>("get_setting", { key: "lineHeight" }).then((v) => v && setLineHeight(Number(v))).catch(() => {});
  }, []);

  const scrollFraction = useCallback((): number => {
    const el = scrollRef.current;
    if (!el) return 0;
    const max = Math.max(1, el.scrollHeight - el.clientHeight);
    return Math.min(1, Math.max(0, el.scrollTop / max));
  }, []);

  // Keep a ref to the latest save fn so every save path (autosave interval,
  // explicit leave) uses the current position, not a stale closure.
  const saveRef = useRef(saveProgress);
  saveRef.current = saveProgress;
  const saveProgressNow = useCallback(() => { saveRef.current(); }, []);

  // Autosave progress every ~5s while reading.
  useEffect(() => {
    if (chapters.length === 0) return;
    const id = setInterval(() => saveRef.current(), 5000);
    return () => clearInterval(id);
  }, [chapters.length]);

  function saveProgress() {
    const total = chapters.length || 1;
    const percent = Math.round(((chapterIndex + scrollFraction()) / total) * 100);
    invoke("save_progress", {
      bookId: book.id,
      chapterIndex,
      scrollOffset: scrollFraction(),
      percent,
    }).catch(() => {});
  }

  // Explicit leave: persist the current position, then go back.
  function leave() {
    saveProgressNow();
    onBack();
  }

  async function go(delta: number) {
    const next = Math.min(chapters.length - 1, Math.max(0, chapterIndex + delta));
    if (next === chapterIndex || turnBusyRef.current) return; // ignore while a slide is in flight
    // Make sure the target page is in hand before starting the slide.
    let toHtml = cacheRef.current.get(next);
    if (toHtml == null) {
      try {
        toHtml = await invoke<string>("get_chapter_html", { bookId: book.id, chapterIndex: next });
      } catch {
        return; // couldn't load it — stay put
      }
      cacheRef.current.set(next, toHtml);
    }
    turnBusyRef.current = true;
    // Freeze at the top so the absolute overlay aligns to the visible area;
    // `off` remembers where you were so the outgoing page shows your spot.
    const scroller = scrollRef.current;
    const off = scroller?.scrollTop ?? 0;
    if (scroller) scroller.scrollTop = 0;
    setSlide({ dir: delta > 0 ? "next" : "prev", outHtml: html, inHtml: toHtml, off, inIndex: next });
  }

  function finishTurn() {
    turnBusyRef.current = false;
    if (slide) {
      // Commit the incoming chapter. Scroll is already 0 and the base will
      // render inHtml at its top — identical to what was on top of the
      // overlay, so nothing jumps when the slide unmounts.
      setChapterIndex(slide.inIndex);
      setHtml(slide.inHtml);
    }
    setSlide(null);
  }

  // --- Drag-to-flip: a horizontal swipe turns the page like turning a book page. ---
  const claimedDrag = useRef(false);
  function onPointerDown(e: React.PointerEvent) {
    dragStart.current = { x: e.clientX, y: e.clientY };
    claimedDrag.current = false;
    setDragging(true);
  }
  function onPointerMove(e: React.PointerEvent) {
    if (!dragStart.current) return;
    const dx = e.clientX - dragStart.current.x;
    const dy = e.clientY - dragStart.current.y;
    // Only claim the gesture once it's clearly horizontal, so vertical scrolling still works.
    if (!claimedDrag.current && Math.abs(dx) > 18 && Math.abs(dx) > Math.abs(dy) * 1.2) {
      claimedDrag.current = true;
    }
  }
  function onPointerUp(e: React.PointerEvent) {
    const start = dragStart.current;
    if (start && claimedDrag.current) {
      const dx = e.clientX - start.x;
      // Drag left → next chapter, drag right → previous chapter.
      if (dx <= -60) go(1);
      else if (dx >= 60) go(-1);
    }
    dragStart.current = null;
    claimedDrag.current = false;
    setDragging(false);
  }
  function onPointerCancel() {
    dragStart.current = null;
    claimedDrag.current = false;
    setDragging(false);
  }

  const themeOrder: ThemeName[] = ["light", "dark", "reading"];
  function cycleTheme() {
    const next = themeOrder[(themeOrder.indexOf(theme) + 1) % themeOrder.length];
    setTheme(next);
  }

  // Jump to a book/chapter/verse from the navigator.
  function jumpTo(flatIndex: number, verse: number) {
    const el = scrollRef.current;
    if (el) el.scrollTop = 0; // reset before the chapter loads
    setChapterIndex(flatIndex);
    setPendingVerse(verse > 0 ? verse : null);
  }

  // Scroll to the requested verse once its HTML is rendered.
  useEffect(() => {
    if (pendingVerse == null || loading) return;
    const v = document.querySelector(`.bible-vnum[data-verse="${pendingVerse}"]`);
    if (v) v.scrollIntoView({ block: "center" });
    setPendingVerse(null);
  }, [html, pendingVerse, loading]);

  async function addBookmarkAtCurrent() {
    await invoke("add_bookmark", {
      bookId: book.id,
      chapterIndex,
      scrollOffset: scrollFraction(),
    });
  }

  return (
    <div className="flex h-full flex-col">
      {/* Top bar */}
      <header
        className="flex items-center justify-between px-4 py-2.5 border-b themed-surface shrink-0"
        style={{ borderColor: "var(--border)", background: "var(--bg-elevated)" }}
      >
        <button onClick={leave} className="text-sm flex items-center gap-1.5 hover:opacity-80 transition-opacity">
          ← Library
        </button>
        <div className="flex-1 text-center px-4 min-w-0">
          <p className="text-sm font-medium truncate">{book.title}</p>
        </div>
        <button onClick={() => setBookmarksOpen(true)} className="text-sm flex items-center gap-1.5 hover:opacity-80 transition-opacity" title="Bookmarks">
          🔖 Bookmarks
        </button>
      </header>

      {/* Content — drag left/right to turn the page */}
      <div className="relative flex-1 min-h-0">
        <div
          ref={scrollRef}
          className="h-full overflow-y-auto select-none"
          style={{ cursor: dragging ? "grabbing" : "grab", touchAction: "pan-y" }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerCancel}
        >
          {loading ? (
            <div className="h-full flex items-center justify-center text-sm" style={{ color: "var(--text-muted)" }}>
              Loading…
            </div>
          ) : error ? (
            <div className="h-full flex items-center justify-center px-8 text-center">
              <p style={{ color: "var(--text-muted)" }}>Could not open this chapter: {error}</p>
            </div>
          ) : (
            <div className="page-stage">
              <article
                className="font-reading mx-auto max-w-[46rem] px-8 py-10"
                style={{ fontSize: `${fontSize}px`, lineHeight }}
                dangerouslySetInnerHTML={{ __html: html }}
              />
            </div>
          )}
        </div>
        {slide && (
          <div className="slide-viewport">
            <div className={`slide-track slide-${slide.dir}`} onAnimationEnd={finishTurn}>
              {slide.dir === "next" ? (
                <>
                  <section className="slide-page" style={{ marginTop: -slide.off }}>
                    <div className="font-reading mx-auto max-w-[46rem] px-8 py-10" style={{ fontSize: `${fontSize}px`, lineHeight }} dangerouslySetInnerHTML={{ __html: slide.outHtml }} />
                  </section>
                  <section className="slide-page">
                    <div className="font-reading mx-auto max-w-[46rem] px-8 py-10" style={{ fontSize: `${fontSize}px`, lineHeight }} dangerouslySetInnerHTML={{ __html: slide.inHtml }} />
                  </section>
                </>
              ) : (
                <>
                  <section className="slide-page">
                    <div className="font-reading mx-auto max-w-[46rem] px-8 py-10" style={{ fontSize: `${fontSize}px`, lineHeight }} dangerouslySetInnerHTML={{ __html: slide.inHtml }} />
                  </section>
                  <section className="slide-page" style={{ marginTop: -slide.off }}>
                    <div className="font-reading mx-auto max-w-[46rem] px-8 py-10" style={{ fontSize: `${fontSize}px`, lineHeight }} dangerouslySetInnerHTML={{ __html: slide.outHtml }} />
                  </section>
                </>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Bottom toolbar */}
      <footer
        className="flex items-center justify-between gap-2 px-4 py-2.5 border-t themed-surface shrink-0"
        style={{ borderColor: "var(--border)", background: "var(--bg-elevated)" }}
      >
        <div className="flex items-center gap-1">
          <ToolButton label="Previous chapter" disabled={chapterIndex === 0} onClick={() => go(-1)}>←</ToolButton>
          <span className="text-xs px-2 tabular-nums" style={{ color: "var(--text-muted)" }}>
            {chapters.length > 0 ? `${chapterIndex + 1} / ${chapters.length}` : ""}
          </span>
          {chapters.length > 0 && (
            <ToolButton label="Go to chapter" onClick={() => setNavOpen(true)}>Go To</ToolButton>
          )}
          <ToolButton label="Next chapter" disabled={chapterIndex >= chapters.length - 1} onClick={() => go(1)}>→</ToolButton>
        </div>

        <div className="flex items-center gap-1">
          <ToolButton label="Decrease font size" disabled={fontSize <= FONT_SIZES[0]} onClick={() => setFontSize((s) => Math.max(FONT_SIZES[0], s - 2))}>A−</ToolButton>
          <ToolButton label="Increase font size" disabled={fontSize >= FONT_SIZES[FONT_SIZES.length - 1]} onClick={() => setFontSize((s) => Math.min(FONT_SIZES[FONT_SIZES.length - 1], s + 2))}>A+</ToolButton>
          <ToolButton label="Cycle line spacing" onClick={() => setLineHeight(LINE_HEIGHTS[(LINE_HEIGHTS.indexOf(lineHeight) + 1) % LINE_HEIGHTS.length])}>¶</ToolButton>
          <ToolButton label={`Text color: ${textColor}`} onClick={cycleTextColor}>A▾</ToolButton>
        </div>

        <button
          onClick={cycleTheme}
          className="rounded-lg px-3 py-1.5 text-xs font-medium transition-opacity hover:opacity-80"
          style={{ background: "var(--accent-soft)", color: "var(--accent)" }}
          title="Cycle theme (Light / Dark / Reading)"
        >
          {theme === "light" ? "☀ Light" : theme === "dark" ? "🌙 Dark" : "📜 Reading"}
        </button>
      </footer>

      <GoToDrawer
        open={navOpen}
        toc={toc}
        chapters={chapters}
        currentChapterIndex={chapterIndex}
        onClose={() => setNavOpen(false)}
        onJump={jumpTo}
      />
      <BookmarkDrawer bookId={book.id} open={bookmarksOpen} onClose={() => setBookmarksOpen(false)} onAdd={addBookmarkAtCurrent} onJump={(ch, off) => {
        if (ch !== chapterIndex) setChapterIndex(ch);
        requestAnimationFrame(() => {
          const el = scrollRef.current;
          if (!el) return;
          const max = Math.max(1, el.scrollHeight - el.clientHeight);
          el.scrollTop = off * max;
        });
      }} />
    </div>
  );
}

function ToolButton({ label, disabled, onClick, children }: { label: string; disabled?: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="rounded-lg px-2.5 py-1 text-sm transition-opacity hover:opacity-80 disabled:opacity-40"
      style={{ background: "var(--bg)" }}
    >
      {children}
    </button>
  );
}
