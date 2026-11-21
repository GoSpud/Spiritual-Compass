import { useEffect, useState } from "react";

export interface BibleTocChapter {
  index: number;
  chapter_no: number;
  verse_count: number;
}

export interface BibleTocBook {
  no: number;
  name: string;
  chapters: BibleTocChapter[];
}

/** Locate a flat reader chapter index within the TOC (book + chapter numbers). */
export function locateInToc(toc: BibleTocBook[] | null, index: number): { book: BibleTocBook; chapter: BibleTocChapter } | null {
  if (!toc) return null;
  for (const b of toc) {
    const ch = b.chapters.find((c) => c.index === index);
    if (ch) return { book: b, chapter: ch };
  }
  return null;
}

interface Props {
  open: boolean;
  /** Non-null → Bible mode (Book/Chapter/Verse dropdowns). */
  toc: BibleTocBook[] | null;
  /** Spine chapters for regular books ({ index, title }). */
  chapters: Array<{ index: number; title: string }>;
  currentChapterIndex: number;
  onClose: () => void;
  onJump: (flatIndex: number, verse: number) => void;
}

/**
 * "Go To" navigator. Bibles get cascading Book → Chapter → Verse dropdowns;
 * regular books (EGW Library, Devotionals, …) get a single chapter dropdown.
 * Right-hand drawer over a backdrop, mirroring the BookmarkDrawer pattern.
 */
export function GoToDrawer({ open, toc, chapters, currentChapterIndex, onClose, onJump }: Props) {
  const isBible = toc != null && toc.length > 0;

  // Bible mode: cascading selects.
  const [bookNo, setBookNo] = useState(0);
  const [chapterNo, setChapterNo] = useState(0);
  const [verse, setVerse] = useState(0);
  // Regular book mode: single chapter select.
  const [selIndex, setSelIndex] = useState(0);

  const books = toc ?? [];

  // Each time it opens, start from where the reader currently is.
  useEffect(() => {
    if (!open) return;
    if (isBible) {
      const here = locateInToc(toc, currentChapterIndex);
      if (here) {
        setBookNo(here.book.no);
        setChapterNo(here.chapter.chapter_no);
      } else {
        setBookNo(books[0]?.no ?? 1);
        setChapterNo(books[0]?.chapters[0]?.chapter_no ?? 1);
      }
      setVerse(0);
    } else {
      setSelIndex(Math.min(currentChapterIndex, Math.max(0, chapters.length - 1)));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  const book = books.find((b) => b.no === bookNo);
  const chapter = book?.chapters.find((c) => c.chapter_no === chapterNo);

  const selectStyle: React.CSSProperties = { background: "var(--bg)", color: "var(--text)", border: "1px solid var(--border)" };

  let body: React.ReactNode;
  let footer: React.ReactNode = null;

  if (isBible) {
    body = (
      <>
        <label className="block">
          <span className="block mb-1 text-xs font-medium" style={{ color: "var(--text-muted)" }}>Book</span>
          <select
            value={bookNo}
            onChange={(e) => {
              const no = Number(e.target.value);
              setBookNo(no);
              const nb = books.find((b) => b.no === no);
              setChapterNo(nb?.chapters[0]?.chapter_no ?? 1);
              setVerse(0);
            }}
            className="w-full rounded-lg px-3 py-2 text-sm"
            style={selectStyle}
          >
            {books.map((b) => (
              <option key={b.no} value={b.no}>{b.name}</option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className="block mb-1 text-xs font-medium" style={{ color: "var(--text-muted)" }}>Chapter</span>
          <select
            value={chapterNo}
            onChange={(e) => { setChapterNo(Number(e.target.value)); setVerse(0); }}
            className="w-full rounded-lg px-3 py-2 text-sm"
            style={selectStyle}
          >
            {(book?.chapters ?? []).map((c) => (
              <option key={c.chapter_no} value={c.chapter_no}>Chapter {c.chapter_no}</option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className="block mb-1 text-xs font-medium" style={{ color: "var(--text-muted)" }}>Verse</span>
          <select
            value={verse}
            onChange={(e) => setVerse(Number(e.target.value))}
            className="w-full rounded-lg px-3 py-2 text-sm"
            style={selectStyle}
          >
            <option value={0}>Top of chapter</option>
            {Array.from({ length: chapter?.verse_count ?? 0 }, (_, i) => i + 1).map((v) => (
              <option key={v} value={v}>{v}</option>
            ))}
          </select>
        </label>
      </>
    );
    footer = (
      <footer className="p-3 border-t" style={{ borderColor: "var(--border)" }}>
        <button
          onClick={() => { if (!chapter) return; onJump(chapter.index, verse); onClose(); }}
          disabled={!chapter}
          className="w-full rounded-lg py-2 text-sm font-medium transition-opacity hover:opacity-80 disabled:opacity-40"
          style={{ background: "var(--accent)", color: "#1c1a17" }}
        >
          {book && chapter ? `Go to ${book.name} ${chapterNo}${verse > 0 ? `:${verse}` : ""}` : "Go"}
        </button>
      </footer>
    );
  } else {
    body = (
      <label className="block">
        <span className="block mb-1 text-xs font-medium" style={{ color: "var(--text-muted)" }}>Chapter</span>
        <select
          value={selIndex}
          onChange={(e) => { onJump(Number(e.target.value), 0); onClose(); }}
          className="w-full rounded-lg px-3 py-2 text-sm"
          style={selectStyle}
        >
          {chapters.map((c, i) => (
            <option key={i} value={c.index}>{c.title || `Chapter ${i + 1}`}</option>
          ))}
        </select>
      </label>
    );
  }

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
          <h2 className="text-sm font-semibold">Go To</h2>
          <button onClick={onClose} className="text-lg leading-none opacity-70 hover:opacity-100">×</button>
        </header>

        <div className="flex-1 overflow-y-auto p-4 space-y-4">{body}</div>
        {footer}
      </aside>
    </div>
  );
}
