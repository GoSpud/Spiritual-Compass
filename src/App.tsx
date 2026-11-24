import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LibraryView } from "./components/Library/LibraryView";
import { ReaderView } from "./components/Reader/ReaderView";
import type { BookWithProgress } from "./types";

// Remember which book was last opened so the library can offer a
// "Continue reading" card. Persisted via the settings KV.
function rememberLastRead(book: BookWithProgress) {
  invoke("set_setting", { key: "lastBookId", value: String(book.id) }).catch(() => {});
}

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
      onOpenBook={(b) => { setInitialChapter(null); setOpenBook(b); rememberLastRead(b); }}
      onOpenAt={(b, ch) => { setInitialChapter(ch); setOpenBook(b); rememberLastRead(b); }}
    />
  );
}
