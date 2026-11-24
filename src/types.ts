export type Category = "EGW Library" | "Devotionals" | "Biographies" | "Bibles";

export const CATEGORIES: Category[] = [
  "EGW Library",
  "Devotionals",
  "Biographies",
  "Bibles",
];

/** User-facing labels for the fixed stored category values. */
export const CATEGORY_LABELS: Record<Category, string> = {
  "EGW Library": "EGW Library",
  "Devotionals": "Devotionals",
  "Biographies": "EGW Biographies",
  "Bibles": "Bibles",
};

export function categoryLabel(c: Category): string {
  return CATEGORY_LABELS[c];
}

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

export interface Highlight {
  id: number;
  book_id: number;
  chapter_index: number;
  scroll_offset: number;
  text: string;
  verse_number: number | null;
  created_at: string;
}

export interface Note {
  id: number;
  book_id: number;
  chapter_index: number;
  scroll_offset: number;
  text: string;
  verse_number: number | null;
  created_at: string;
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

/** One deterministic verse to feature on the library's daily card. */
export interface DailyVerse {
  book_id: number;
  book_title: string;
  book_name: string;
  chapter_index: number;
  chapter_no: number;
  verse_number: number;
  text: string;
}
