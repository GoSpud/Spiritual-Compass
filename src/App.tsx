import { useState } from "react";
import { LibraryView } from "./components/Library/LibraryView";
import { ReaderView } from "./components/Reader/ReaderView";
import type { BookWithProgress } from "./types";

export default function App() {
  const [openBook, setOpenBook] = useState<BookWithProgress | null>(null);
  // When set, the reader opens at this chapter instead of the saved position.
  const [initialChapter, setInitialChapter] = useState<number | null>(null);

  return openBook ? (
    <ReaderView
      book={openBook}
      initialChapter={initialChapter}
      onBack={() => { setOpenBook(null); setInitialChapter(null); }}
    />
  ) : (
    <LibraryView
      onOpenBook={(b) => { setInitialChapter(null); setOpenBook(b); }}
      onOpenAt={(b, ch) => { setInitialChapter(ch); setOpenBook(b); }}
    />
  );
}
