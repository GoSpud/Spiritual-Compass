export type Category = "EGW Library" | "Devotionals" | "Biographies" | "Bibles";

export const CATEGORIES: Category[] = [
  "EGW Library",
  "Devotionals",
  "Biographies",
  "Bibles",
];

export interface Book {
  id: number;
  title: string;
  author: string;
  category: Category;
  file_path: string;
  cover_path: string | null;
  added_at: string;
}

export interface ReadingProgress {
  book_id: number;
  chapter_index: number;
  scroll_offset: number;
  percent: number;
  updated_at: string;
}

export interface Bookmark {
  id: number;
  book_id: number;
  chapter_index: number;
  scroll_offset: number;
  created_at: string;
}

/** A book with its progress (nullable) joined in. */
export interface BookWithProgress extends Book {
  percent: number | null;
}

/** One full-text search hit across the library. */
export interface SearchHit {
  book_id: number;
  title: string;
  author: string;
  category: Category;
  matchType: "title" | "author" | "text";
  chapterIndex: number;
  snippet: string; // contains <mark>…</mark> highlighting
}
