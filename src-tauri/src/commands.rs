//! Tauri commands: books, import, reader, progress, bookmarks, settings.

use std::cmp::Ordering;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::Serialize;

use tauri::State;

use crate::bible;
use crate::db::Db;
use crate::epub_parser;

/// Shared app state managed by Tauri.
pub struct AppState {
    pub db: Db,
    /// App data directory (books/, covers/ live under it).
    pub data_dir: PathBuf,
    /// Normalized Bible models, cached per book id so page turns never re-parse.
    pub bibles: std::sync::Mutex<std::collections::HashMap<i64, Vec<bible::Book>>>,
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct BookRow {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub category: String,
    pub file_path: String,
    pub cover_path: Option<String>,
    pub added_at: String,
}

#[derive(Serialize)]
pub struct BookWithProgress {
    #[serde(flatten)]
    pub book: BookRow,
    /// 0..100 or null when the book has never been opened.
    pub percent: Option<f64>,
}

#[derive(Serialize)]
pub struct ChapterInfo {
    pub index: usize,
    pub title: String,
    pub href: String,
}

/// One book of a normalized Bible, for the Book/Chapter/Verse navigator.
#[derive(Serialize)]
pub struct TocBook {
    pub no: u32,
    pub name: String,
    pub chapters: Vec<TocChapter>,
}

#[derive(Serialize)]
pub struct TocChapter {
    /// Flat reader index (books concatenated in order) — what the reader navigates by.
    pub index: usize,
    #[serde(rename = "chapter_no")]
    pub chapter_no: u32,
    #[serde(rename = "verse_count")]
    pub verse_count: usize,
}

#[derive(Serialize)]
pub struct ProgressRow {
    pub book_id: i64,
    pub chapter_index: i32,
    pub scroll_offset: f64,
    pub percent: f64,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct BookmarkRow {
    pub id: i64,
    pub book_id: i64,
    pub chapter_index: i32,
    pub scroll_offset: f64,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct HighlightRow {
    pub id: i64,
    pub book_id: i64,
    pub chapter_index: i32,
    pub scroll_offset: f64,
    pub text: String,
    pub verse_number: Option<i32>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct NoteRow {
    pub id: i64,
    pub book_id: i64,
    pub chapter_index: i32,
    pub scroll_offset: f64,
    pub text: String,
    pub verse_number: Option<i32>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct ImportResult {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub category: String,
}

/// One deterministic verse to feature on the library's daily card.
#[derive(Serialize)]
pub struct DailyVerse {
    pub book_id: i64,
    pub book_title: String,
    pub book_name: String,
    pub chapter_index: usize,
    pub chapter_no: u32,
    pub verse_number: u32,
    pub text: String,
}

/// Convert a rusqlite error into our command error type (a plain `String`).
fn db_err(e: rusqlite::Error) -> String {
    e.to_string()
}

/// Case-insensitive natural comparison of titles: runs of digits compare as numbers,
/// so "Volume 2" sorts before "Volume 10" while plain text stays alphabetical.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let mut na: u128 = 0;
            while i < a.len() && a[i].is_ascii_digit() {
                na = na.saturating_mul(10).saturating_add(a[i].to_digit(10).unwrap() as u128);
                i += 1;
            }
            let mut nb: u128 = 0;
            while j < b.len() && b[j].is_ascii_digit() {
                nb = nb.saturating_mul(10).saturating_add(b[j].to_digit(10).unwrap() as u128);
                j += 1;
            }
            match na.cmp(&nb) {
                Ordering::Equal => {}
                other => return other,
            }
        } else if a[i] != b[j] {
            return a[i].cmp(&b[j]);
        } else {
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

// ---------------------------------------------------------------------------
// Books
// ---------------------------------------------------------------------------

fn fetch_books(conn: &rusqlite::Connection) -> Result<Vec<BookWithProgress>, String> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.title, b.author, b.category, b.file_path, b.cover_path, b.added_at,
                COALESCE(p.percent, NULL) AS percent
         FROM books b LEFT JOIN reading_progress p ON p.book_id = b.id
         ORDER BY b.added_at DESC",
    ).map_err(db_err)?;
    let rows = stmt.query_map((), |row| {
        Ok(BookWithProgress {
            book: BookRow {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                category: row.get(3)?,
                file_path: row.get(4)?,
                cover_path: row.get(5)?,
                added_at: row.get(6)?,
            },
            percent: row.get(7)?,
        })
    }).map_err(db_err)?;
    let mut out = Vec::new();
    for r in rows { out.push(r.map_err(db_err)?); }
    Ok(out)
}

#[tauri::command]
pub fn list_books(state: State<'_, AppState>, category: Option<String>, search: Option<String>) -> Result<Vec<BookWithProgress>, String> {
    let conn = state.db.lock();
    let mut all = fetch_books(&conn)?;
    if let Some(cat) = category {
        all.retain(|b| b.book.category == cat);
    }
    if let Some(q) = search {
        let q = q.to_lowercase();
        all.retain(|b| {
            b.book.title.to_lowercase().contains(&q) || b.book.author.to_lowercase().contains(&q)
        });
    }
    // Alphabetical with natural number ordering (Volume 2 before Volume 10).
    all.sort_by(|x, y| natural_cmp(&x.book.title, &y.book.title));
    Ok(all)
}

#[tauri::command]
pub fn get_book(state: State<'_, AppState>, id: i64) -> Result<Option<BookWithProgress>, String> {
    let conn = state.db.lock();
    Ok(fetch_books(&conn)?.into_iter().find(|b| b.book.id == id))
}

#[tauri::command]
pub fn delete_book(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    // Remove the stored .epub and cover files, then the row (cascades progress/bookmarks).
    let conn = state.db.lock();
    if let Ok((file_path, cover_path)) = conn.query_row(
        "SELECT file_path, cover_path FROM books WHERE id = ?",
        [id],
        |r| {
            let fp: String = r.get(0)?;
            let cp: Option<String> = r.get(1)?;
            Ok((fp, cp))
        },
    ) {
        // Only delete files that live inside the app data dir.
        let in_data_dir = |p: &str| -> bool {
            PathBuf::from(p).strip_prefix(&state.data_dir).is_ok()
        };
        if in_data_dir(&file_path) {
            let _ = std::fs::remove_file(&file_path);
        }
        if let Some(c) = cover_path {
            if in_data_dir(&c) {
                let _ = std::fs::remove_file(&c);
            }
        }
    }
    conn.execute("DELETE FROM books WHERE id = ?", [id]).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn update_book_category(state: State<'_, AppState>, id: i64, category: String) -> Result<(), String> {
    let valid = ["EGW Library", "Devotionals", "Biographies", "Bibles"];
    if !valid.iter().any(|v| *v == category) {
        return Err(format!("unknown category '{}'", category));
    }
    let conn = state.db.lock();
    conn.execute("UPDATE books SET category = ? WHERE id = ?", (category, id)).map_err(db_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

/// Import a single EPUB from `src_path` into the library. Shared by the user-facing
/// `import_epubs` command and first-launch seeding so both stay byte-for-byte identical.
/// When `index` is true the book's chapter text is also stored for full-text search;
/// seeding passes false to keep first launch fast (search indexes lazily on first use).
pub fn import_book(db: &Db, data_dir: &std::path::Path, src_path: &str, category: &str, index: bool) -> Result<ImportResult, String> {
    let cat = category;
    // Parse metadata + cover from the source file (before copying it in).
    let parsed = epub_parser::parse_epub(src_path)?;
    // Bibles often ship with no OPF title — name them after the original file
    // (e.g. "King James Version.epub" -> "King James Version").
    let fname = src_path.rsplit(['/', '\\']).next().unwrap_or("Unknown Title");
    let stem = if let Some(s) = fname.strip_suffix(".epub") { s }
                else if let Some(s) = fname.strip_suffix(".epub.zip") { s }
                else { fname };
    let title = if !parsed.title.is_empty() {
        parsed.title
    } else if cat == "Bibles" {
        stem.to_string()
    } else {
        "Unknown Title".into()
    };
    let author = parsed.author;

    // Insert the book row first so we get a unique id to name files by.
    // file_path is filled in right after the copy below.
    let conn = db.lock();
    let mut stmt = conn.prepare("INSERT INTO books (title, author, category, file_path) VALUES (?, ?, ?, ?)").map_err(db_err)?;
    stmt.execute((title.clone(), author.clone(), cat.to_string(), String::new())).map_err(db_err)?;
    let id = conn.last_insert_rowid();

    // Copy into the app data dir so the library is self-contained. Name the file
    // after the book's own row id (like covers) so re-importing can never
    // overwrite an existing book's file — each import gets a fresh, unique id.
    let books_dir = data_dir.join("books");
    std::fs::create_dir_all(&books_dir).map_err(|e| e.to_string())?;
    let dest = books_dir.join(format!("book_{}.epub", id));
    std::fs::copy(src_path, &dest).map_err(|e| format!("could not copy {}: {}", src_path, e))?;
    conn.execute("UPDATE books SET file_path = ? WHERE id = ?", (dest.to_string_lossy().to_string(), id)).map_err(db_err)?;

    // Save the cover named after the book id.
    if let Some(bytes) = &parsed.cover_bytes {
        let covers_dir = data_dir.join("covers");
        std::fs::create_dir_all(&covers_dir).map_err(|e| e.to_string())?;
        let ext = parsed.cover_ext.clone().unwrap_or_else(|| "jpg".into());
        let cover_file = covers_dir.join(format!("{}.{}", id, ext));
        std::fs::write(&cover_file, bytes).map_err(|e| e.to_string())?;
        conn.execute("UPDATE books SET cover_path = ? WHERE id = ?", (cover_file.to_string_lossy().to_string(), id)).map_err(db_err)?;
    }

    // Index each chapter's plain text so the library can full-text search inside books.
    // (Passes the already-locked connection — must NOT re-lock or we deadlock.)
    if index {
        index_book_for_search(&*conn, &dest, cat, id);
    }

    Ok(ImportResult {
        id,
        title: title.clone(),
        author: author.clone(),
        category: cat.to_string(),
    })
}

#[tauri::command]
pub fn import_epubs(state: State<'_, AppState>, paths: Vec<String>, category: Option<String>) -> Result<Vec<ImportResult>, String> {
    let cat = category.unwrap_or_else(|| "EGW Library".to_string());
    if !["EGW Library", "Devotionals", "Biographies", "Bibles"].iter().any(|v| *v == cat) {
        return Err(format!("unknown category '{}'", cat));
    }
    let mut out = Vec::new();
    for path in &paths {
        if !path.to_lowercase().ends_with(".epub") && !path.to_lowercase().ends_with(".epub.zip") {
            continue;
        }
        match import_book(&state.db, &state.data_dir, path, &cat, true) {
            Ok(r) => out.push(r),
            Err(e) => eprintln!("import failed for {}: {}", path, e),
        }
    }
    Ok(out)
}

/// On first launch (empty library) seed the app with the full embedded library so a
/// fresh copy of the exe already contains every book. Reuses `import_book` with
/// indexing off to keep startup fast; returns the number of books seeded.
pub fn seed_library_if_empty(db: &Db, data_dir: &std::path::Path) -> Result<usize, String> {
    let conn = db.lock();
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM books", [], |r| r.get(0)).map_err(db_err)?;
    drop(conn);
    if count > 0 {
        return Ok(0); // already populated — nothing to seed.
    }

    let tmp = std::env::temp_dir().join(format!("sc_seed_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let mut seeded = 0usize;
    for (bytes, category) in crate::embedded_library::books() {
        // Materialize the embedded bytes to a temp file so import_book can parse + copy.
        let tmp_path = tmp.join(format!("book_{}.epub", seeded));
        std::fs::write(&tmp_path, bytes).map_err(|e| e.to_string())?;
        if let Ok(_r) = import_book(db, data_dir, &tmp_path.to_string_lossy(), category, false) {
            seeded += 1;
        } else {
            eprintln!("seed failed for embedded book {}", seeded);
        }
        let _ = std::fs::remove_file(&tmp_path);
    }

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(seeded)
}

/// Store each chapter's plain text in `book_chapters` for full-text search.
/// Takes an already-locked connection (callers hold the lock); must NOT re-lock or we deadlock.
fn index_book_for_search(conn: &rusqlite::Connection, path: &PathBuf, category: &str, book_id: i64) {
    // Bibles are indexed per real chapter so a search hit opens at the right place.
    let texts: Vec<String> = if category == "Bibles" {
        match build_bible_model(path.to_str().unwrap()) {
            Some(books) => {
                let mut t = Vec::new();
                for b in &books { for ch in &b.chapters { t.push(bible::chapter_text(b, ch)); } }
                t
            }
            None => match epub_parser::all_chapter_texts(path.to_str().unwrap(), &parsed_chapters(path)) {
                Ok(t) => t,
                Err(e) => { eprintln!("could not index chapters for search: {}", e); return; }
            },
        }
    } else {
        match epub_parser::all_chapter_texts(path.to_str().unwrap(), &parsed_chapters(path)) {
            Ok(t) => t,
            Err(e) => { eprintln!("could not index chapters for search: {}", e); return; }
        }
    };

    let mut stmt = match conn.prepare("INSERT INTO book_chapters (book_id, chapter_index, text) VALUES (?, ?, ?)") {
        Ok(s) => s,
        Err(e) => { eprintln!("could not prepare index insert: {}", e); return; }
    };
    for (i, text) in texts.iter().enumerate() {
        if let Err(e) = stmt.execute((book_id, i as i32, text.as_str())) {
            eprintln!("index row {} failed: {}", i, e);
        }
    }
}

/// Convenience: the spine chapters of a book's EPUB (for raw-text indexing fallback).
fn parsed_chapters(path: &PathBuf) -> Vec<epub_parser::Chapter> {
    match epub_parser::parse_epub(path.to_str().unwrap()) {
        Ok(p) => p.chapters,
        Err(e) => { eprintln!("could not parse for indexing: {}", e); Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Full-text search across the whole library (title + author + chapter content).
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SearchHit {
    pub book_id: i64,
    pub title: String,
    pub author: String,
    pub category: String,
    /// Where the match lives: "title" | "author" | "text".
    #[serde(rename = "matchType")]
    pub match_type: String,
    /// 0-based chapter index (only meaningful when match_type == "text").
    #[serde(rename = "chapterIndex")]
    pub chapter_index: i32,
    /// A short snippet around the match with <mark>…</mark> highlighting.
    pub snippet: String,
}

#[tauri::command]
pub fn search_books(state: State<'_, AppState>, query: String) -> Result<Vec<SearchHit>, String> {
    let q = query.trim();
    if q.is_empty() { return Ok(Vec::new()); }
    let needle = q.to_lowercase();

    let conn = state.db.lock();

    // All books — file_path lets us lazily index books imported before search existed.
    let mut stmt = conn.prepare("SELECT id, title, author, category, file_path FROM books").map_err(db_err)?;
    let row_iter = stmt.query_map((), |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    }).map_err(db_err)?;
    let mut books: Vec<(i64, String, String, String, String)> = Vec::new();
    for row in row_iter {
        books.push(row.map_err(db_err)?);
    }

    // Chapter index grouped by book.
    let mut stmt2 = conn.prepare("SELECT book_id, chapter_index, text FROM book_chapters ORDER BY book_id, chapter_index").map_err(db_err)?;
    let crow_iter = stmt2.query_map((), |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    }).map_err(db_err)?;
    drop(stmt);
    let mut chapters_by_book: std::collections::HashMap<i64, Vec<(i32, String)>> = std::collections::HashMap::new();
    for row in crow_iter {
        let (bid, ci, text) = row.map_err(db_err)?;
        chapters_by_book.entry(bid).or_default().push((ci, text));
    }

    // Lazily index any book imported before full-text search existed.
    for (id, _t, _a, category, file_path) in &books {
        if !chapters_by_book.contains_key(id) {
            let path_buf = PathBuf::from(file_path);
            index_book_for_search(&*conn, &path_buf, category, *id);
            // Re-read the freshly-indexed rows so this search can use them.
            let mut s = conn.prepare("SELECT chapter_index, text FROM book_chapters WHERE book_id = ? ORDER BY chapter_index").map_err(db_err)?;
            let it = s.query_map([*id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(db_err)?;
            let mut texts: Vec<(i32, String)> = Vec::new();
            for row in it { texts.push(row.map_err(db_err)?); }
            chapters_by_book.insert(*id, texts);
        }
    }

    let mut hits: Vec<SearchHit> = Vec::new();
    for (id, title, author, category, _fp) in &books {
        if title.to_lowercase().contains(&needle) {
            hits.push(SearchHit {
                book_id: *id, match_type: "title".into(), chapter_index: 0,
                snippet: highlight_first(title, &needle), title: title.clone(), author: author.clone(), category: category.clone(),
            });
        } else if author.to_lowercase().contains(&needle) {
            hits.push(SearchHit {
                book_id: *id, match_type: "author".into(), chapter_index: 0,
                snippet: highlight_first(author, &needle), title: title.clone(), author: author.clone(), category: category.clone(),
            });
        }
        // Full-text: first matching chapter surfaces the book.
        if let Some(chs) = chapters_by_book.get(id) {
            for (ci, text) in chs {
                if let Some(snippet) = snippet_around(text, &needle) {
                    hits.push(SearchHit {
                        book_id: *id, match_type: "text".into(), chapter_index: *ci,
                        snippet, title: title.clone(), author: author.clone(), category: category.clone(),
                    });
                    break;
                }
            }
        }
    }

    Ok(hits)
}

/// Wrap the first case-insensitive occurrence of `needle` in <mark>.
fn highlight_first(text: &str, needle_lower: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let n: Vec<char> = needle_lower.chars().collect();
    if n.is_empty() || lower.len() < n.len() { return text.to_string(); }
    let start = (0..=lower.len() - n.len()).find(|&i| {
        for k in 0..n.len() { if lower[i + k] != n[k] { return false; } }
        true
    });
    match start {
        Some(s) => {
            let mut out = String::new();
            out.push_str(&chars[..s].iter().collect::<String>());
            out.push_str("<mark>");
            out.extend(chars[s..s + n.len()].iter());
            out.push_str("</mark>");
            out.extend(chars[s + n.len()..].iter());
            out
        }
        None => text.to_string(),
    }
}

/// Build a short snippet centered on the first case-insensitive match, with <mark>.
fn snippet_around(text: &str, needle_lower: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let n: Vec<char> = needle_lower.chars().collect();
    if n.is_empty() || lower.len() < n.len() { return None; }
    let start = (0..=lower.len() - n.len()).find(|&i| {
        for k in 0..n.len() { if lower[i + k] != n[k] { return false; } }
        true
    })?;
    const WIN_BEFORE: usize = 45;
    let begin = start.saturating_sub(WIN_BEFORE);
    let end = (start + n.len() + 60).min(chars.len());
    let mut out = String::new();
    if begin > 0 { out.push_str("…"); }
    for i in begin..end {
        if i == start { out.push_str("<mark>"); }
        out.push(chars[i]);
        if i + 1 == start + n.len() { out.push_str("</mark>"); }
    }
    if end < chars.len() { out.push_str("…"); }
    Some(out)
}

/// Deterministic daily pick: the same verse is returned all day, advancing one
/// verse per calendar day (seeded by days-since-epoch). Returned from the first
/// Bible in the library; `None` when there is no Bible or its file cannot be parsed.
#[tauri::command]
pub fn daily_verse(state: State<'_, AppState>) -> Result<Option<DailyVerse>, String> {
    let conn = state.db.lock();
    let row: Result<(i64, String, String), _> = conn.query_row(
        "SELECT id, file_path, title FROM books WHERE category = 'Bibles' ORDER BY id LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    );
    drop(conn);
    let Ok((book_id, file_path, book_title)) = row else {
        return Ok(None);
    };

    let Some(books) = bible_model(&state, book_id, &file_path) else {
        return Ok(None);
    };
    if books.is_empty() {
        return Ok(None);
    }

    // Flatten every verse into candidates with the FLAT reader index,
    // matching get_chapters / get_bible_toc.
    let mut candidates: Vec<(usize, String, u32, u32, String)> = Vec::new();
    let mut flat_index = 0usize;
    for b in &books {
        for ch in &b.chapters {
            for v in &ch.verses {
                candidates.push((flat_index, b.name.clone(), ch.chapter_no, v.number, v.text.clone()));
            }
            flat_index += 1;
        }
    }
    if candidates.is_empty() {
        return Ok(None);
    }

    // One deterministic pick per day (days-since-epoch as the seed).
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400;
    let (flat_idx, book_name, chapter_no, verse_number, text) = &candidates[(days as usize) % candidates.len()];
    Ok(Some(DailyVerse {
        book_id,
        book_title,
        book_name: book_name.clone(),
        chapter_index: *flat_idx,
        chapter_no: *chapter_no,
        verse_number: *verse_number,
        text: text.clone(),
    }))
}

// ---------------------------------------------------------------------------
// Reader (chapters + chapter HTML)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_chapters(state: State<'_, AppState>, book_id: i64) -> Result<Vec<ChapterInfo>, String> {
    let conn = state.db.lock();
    let (file_path, category): (String, String) = conn
        .query_row("SELECT file_path, category FROM books WHERE id = ?", [book_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|_| "book not found".to_string())?;
    drop(conn);

    // Bibles get real per-chapter navigation from the normalized model.
    if category == "Bibles" {
        if let Some(books) = bible_model(&state, book_id, &file_path) {
            let mut out = Vec::new();
            for b in &books {
                for ch in &b.chapters {
                    out.push(ChapterInfo { index: out.len(), title: b.chapter_title(ch), href: String::new() });
                }
            }
            return Ok(out);
        }
    }

    let parsed = epub_parser::parse_epub(&file_path)?;
    Ok(parsed.chapters.into_iter().enumerate().map(|(i, c)| ChapterInfo {
        index: i,
        title: c.title,
        href: c.href,
    }).collect())
}

/// Book/Chapter/Verse table of contents for a normalized Bible.
#[tauri::command]
pub fn get_bible_toc(state: State<'_, AppState>, book_id: i64) -> Result<Vec<TocBook>, String> {
    let conn = state.db.lock();
    let (file_path, category): (String, String) = conn
        .query_row("SELECT file_path, category FROM books WHERE id = ?", [book_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|_| "book not found".to_string())?;
    drop(conn);

    if category != "Bibles" { return Ok(Vec::new()); }

    let Some(books) = bible_model(&state, book_id, &file_path) else { return Ok(Vec::new()); };
    let mut out: Vec<TocBook> = Vec::new();
    for b in &books {
        let mut chapters = Vec::new();
        for ch in &b.chapters {
            chapters.push(TocChapter { index: out.iter().map(|x| x.chapters.len()).sum::<usize>() + chapters.len(), chapter_no: ch.chapter_no, verse_count: ch.verses.len() });
        }
        out.push(TocBook { no: b.no, name: b.name.clone(), chapters });
    }
    Ok(out)
}

#[tauri::command]
pub fn get_chapter_html(state: State<'_, AppState>, book_id: i64, chapter_index: usize) -> Result<String, String> {
    let conn = state.db.lock();
    let (file_path, category): (String, String) = conn
        .query_row("SELECT file_path, category FROM books WHERE id = ?", [book_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|_| "book not found".to_string())?;
    drop(conn);

    // Bibles render from the normalized model: one clean page per real chapter.
    if category == "Bibles" {
        if let Some(books) = bible_model(&state, book_id, &file_path) {
            let mut idx = 0usize;
            for b in &books {
                for ch in &b.chapters {
                    if idx == chapter_index { return Ok(bible::render_chapter_html(b, ch)); }
                    idx += 1;
                }
            }
            return Err("chapter index out of range".into());
        }
    }

    // Non-Bibles (or an unrecognized Bible layout): raw spine rendering.
    let parsed = epub_parser::parse_epub(&file_path)?;
    if chapter_index >= parsed.chapters.len() {
        return Err("chapter index out of range".into());
    }
    Ok(epub_parser::read_chapter_html(&file_path, &parsed.chapters[chapter_index].href)?)
}

/// Build (and cache) the normalized chapter model for a Bible book.
fn bible_model(state: &AppState, book_id: i64, file_path: &str) -> Option<Vec<bible::Book>> {
    let mut cache = state.bibles.lock().expect("bibles cache poisoned");
    if let Some(m) = cache.get(&book_id) { return Some(m.clone()); }

    let model = build_bible_model(file_path);
    // Keep a few Bibles warm (re-opening is instant) without holding the whole library.
    if model.is_some() && cache.len() >= 8 { cache.clear(); }
    if let Some(m) = &model { cache.insert(book_id, m.clone()); }
    model
}

/// Parse an EPUB into a canonical book/chapter/verse model. Tries the whole-Bible
/// formats first (one file holding the entire Bible), then per-book inline markers.
pub fn build_bible_model(file_path: &str) -> Option<Vec<bible::Book>> {
    let parsed = epub_parser::parse_epub(file_path).ok()?;

    // Read every spine HTML file once into memory (avoids re-opening the zip).
    let mut files: Vec<String> = Vec::new();
    for ch in &parsed.chapters {
        if let Ok(html) = epub_parser::read_chapter_html(file_path, &ch.href) { files.push(html); }
    }

    // Whole-bible in one file (escaped-XML or calibre layouts).
    for html in &files {
        if let Some(books) = bible::normalize_whole_bible(html) { return Some(books); }
    }

    // Per-book files (KJV-style): each spine item is one book; keep the ones that parse.
    let mut books: Vec<bible::Book> = Vec::new();
    for html in &files {
        if let Some(chapters) = bible::parse_inline_book(html) {
            let no = (books.len() + 1) as u32;
            books.push(bible::Book {
                no,
                name: bible::book_name(no).unwrap_or("Unknown").to_string(),
                chapters,
            });
        }
    }
    if books.is_empty() { None } else { Some(books) }
}

// ---------------------------------------------------------------------------
// Reading progress
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn save_progress(state: State<'_, AppState>, book_id: i64, chapter_index: i32, scroll_offset: f64, percent: f64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT INTO reading_progress (book_id, chapter_index, scroll_offset, percent, updated_at)
         VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(book_id) DO UPDATE SET
            chapter_index = excluded.chapter_index,
            scroll_offset = excluded.scroll_offset,
            percent = excluded.percent,
            updated_at = CURRENT_TIMESTAMP",
        (book_id, chapter_index, scroll_offset, percent),
    ).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn get_progress(state: State<'_, AppState>, book_id: i64) -> Result<Option<ProgressRow>, String> {
    let conn = state.db.lock();
    match conn.query_row(
        "SELECT chapter_index, scroll_offset, percent, updated_at FROM reading_progress WHERE book_id = ?",
        [book_id],
        |r| Ok(ProgressRow {
            book_id,
            chapter_index: r.get(0)?,
            scroll_offset: r.get(1)?,
            percent: r.get(2)?,
            updated_at: r.get(3)?,
        }),
    ) {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Bookmarks
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn add_bookmark(state: State<'_, AppState>, book_id: i64, chapter_index: i32, scroll_offset: f64) -> Result<BookmarkRow, String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT INTO bookmarks (book_id, chapter_index, scroll_offset) VALUES (?, ?, ?)",
        (book_id, chapter_index, scroll_offset),
    ).map_err(db_err)?;
    let id = conn.last_insert_rowid();
    Ok(conn.query_row("SELECT id, book_id, chapter_index, scroll_offset, created_at FROM bookmarks WHERE id = ?", [id], |r| {
        Ok(BookmarkRow {
            id: r.get(0)?,
            book_id: r.get(1)?,
            chapter_index: r.get(2)?,
            scroll_offset: r.get(3)?,
            created_at: r.get(4)?,
        })
    }).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn list_bookmarks(state: State<'_, AppState>, book_id: i64) -> Result<Vec<BookmarkRow>, String> {
    let conn = state.db.lock();
    let mut stmt = conn.prepare("SELECT id, book_id, chapter_index, scroll_offset, created_at FROM bookmarks WHERE book_id = ? ORDER BY created_at DESC").map_err(db_err)?;
    let rows = stmt.query_map([book_id], |r| {
        Ok(BookmarkRow {
            id: r.get(0)?,
            book_id: r.get(1)?,
            chapter_index: r.get(2)?,
            scroll_offset: r.get(3)?,
            created_at: r.get(4)?,
        })
    }).map_err(db_err)?;
    let mut out = Vec::new();
    for r in rows { out.push(r.map_err(db_err)?); }
    Ok(out)
}

#[tauri::command]
pub fn delete_bookmark(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM bookmarks WHERE id = ?", [id]).map_err(db_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Highlights & notes
// ---------------------------------------------------------------------------

fn map_highlight_row(r: &rusqlite::Row) -> rusqlite::Result<HighlightRow> {
    Ok(HighlightRow {
        id: r.get(0)?,
        book_id: r.get(1)?,
        chapter_index: r.get(2)?,
        scroll_offset: r.get(3)?,
        text: r.get(4)?,
        verse_number: r.get(5)?,
        created_at: r.get(6)?,
    })
}

fn map_note_row(r: &rusqlite::Row) -> rusqlite::Result<NoteRow> {
    Ok(NoteRow {
        id: r.get(0)?,
        book_id: r.get(1)?,
        chapter_index: r.get(2)?,
        scroll_offset: r.get(3)?,
        text: r.get(4)?,
        verse_number: r.get(5)?,
        created_at: r.get(6)?,
    })
}

#[tauri::command]
pub fn add_highlight(state: State<'_, AppState>, book_id: i64, chapter_index: i32, scroll_offset: f64, text: String, verse_number: Option<i32>) -> Result<HighlightRow, String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT INTO highlights (book_id, chapter_index, scroll_offset, text, verse_number) VALUES (?, ?, ?, ?, ?)",
        (book_id, chapter_index, scroll_offset, text, verse_number),
    ).map_err(db_err)?;
    let id = conn.last_insert_rowid();
    Ok(conn.query_row("SELECT id, book_id, chapter_index, scroll_offset, text, verse_number, created_at FROM highlights WHERE id = ?", [id], map_highlight_row).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn list_highlights(state: State<'_, AppState>, book_id: i64) -> Result<Vec<HighlightRow>, String> {
    let conn = state.db.lock();
    let mut stmt = conn.prepare("SELECT id, book_id, chapter_index, scroll_offset, text, verse_number, created_at FROM highlights WHERE book_id = ? ORDER BY created_at DESC").map_err(db_err)?;
    let rows = stmt.query_map([book_id], map_highlight_row).map_err(db_err)?;
    let mut out = Vec::new();
    for r in rows { out.push(r.map_err(db_err)?); }
    Ok(out)
}

#[tauri::command]
pub fn delete_highlight(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM highlights WHERE id = ?", [id]).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn add_note(state: State<'_, AppState>, book_id: i64, chapter_index: i32, scroll_offset: f64, text: String, verse_number: Option<i32>) -> Result<NoteRow, String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT INTO notes (book_id, chapter_index, scroll_offset, text, verse_number) VALUES (?, ?, ?, ?, ?)",
        (book_id, chapter_index, scroll_offset, text, verse_number),
    ).map_err(db_err)?;
    let id = conn.last_insert_rowid();
    Ok(conn.query_row("SELECT id, book_id, chapter_index, scroll_offset, text, verse_number, created_at FROM notes WHERE id = ?", [id], map_note_row).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn list_notes(state: State<'_, AppState>, book_id: i64) -> Result<Vec<NoteRow>, String> {
    let conn = state.db.lock();
    let mut stmt = conn.prepare("SELECT id, book_id, chapter_index, scroll_offset, text, verse_number, created_at FROM notes WHERE book_id = ? ORDER BY created_at DESC").map_err(db_err)?;
    let rows = stmt.query_map([book_id], map_note_row).map_err(db_err)?;
    let mut out = Vec::new();
    for r in rows { out.push(r.map_err(db_err)?); }
    Ok(out)
}

#[tauri::command]
pub fn delete_note(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM notes WHERE id = ?", [id]).map_err(db_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    let conn = state.db.lock();
    match conn.query_row("SELECT value FROM settings WHERE key = ?", [key], |r| r.get(0)) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(db_err(e)),
    }
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    ).map_err(db_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_ordering() {
        assert_eq!(natural_cmp("Volume 2", "Volume 10"), Ordering::Less);
        assert_eq!(natural_cmp("Volume 10", "Volume 9"), Ordering::Greater);
        assert_eq!(natural_cmp("a", "A"), Ordering::Equal);
        assert_eq!(natural_cmp("The Acts of the Apostles", "the acts of the apostles"), Ordering::Equal);
    }

    /// Real-data regression: run the full normalization pipeline against every Bible
    /// EPUB in the user's library and assert canonical structure (66 books / 1189
    /// chapters, correct first/last book names). Skips gracefully if a file is absent.
    #[test]
    fn normalizes_real_bible_epubs() {
        let dir = r"C:\Users\Spud\Documents\AI Projects\Books\Bibles";
        for name in [
            "King James Version.epub",
            "New International Version.epub",
            "Christian Standard Bible.epub",
            "English Standard Version.epub",
            "New King James Version.epub",
            "New American Standard Bible-1995.epub",
            "New Revised Standard Version.epub",
            "Revised Standard Version.epub",
            "Legacy Standard Bible.epub",
        ] {
            let path = format!("{}/{}", dir, name);
            if !std::path::Path::new(&path).exists() { eprintln!("skipping {}: not found", name); continue; }

            let model = build_bible_model(&path).unwrap_or_else(|| panic!("{}: failed to normalize", name));
            assert!(!model.is_empty(), "{}: empty model", name);

            // Every translation must resolve to the full 66-book canon.
            assert_eq!(model.len(), 66, "{}: expected 66 books, got {}", name, model.len());
            let total_chapters: usize = model.iter().map(|b| b.chapters.len()).sum();
            assert_eq!(total_chapters, 1189, "{}: expected 1189 chapters, got {}", name, total_chapters);
            // And a full canon's worth of verses (~29k–31.5k depending on translation) —
            // catches broken verse splits without pinning an exact number.
            let total_verses: usize = model.iter().flat_map(|b| b.chapters.iter()).map(|c| c.verses.len()).sum();
            assert!((28_000..=32_500).contains(&total_verses), "{}: verse count {} out of range", name, total_verses);

            // Spot-check canonical book names and a known chapter count.
            let genesis = &model[0];
            assert_eq!(genesis.name, "Genesis", "{}: first book should be Genesis", name);
            assert!(!genesis.chapters.is_empty(), "{}: Genesis has no chapters", name);
            let revelation = &model[65];
            assert_eq!(revelation.name, "Revelation", "{}: last book should be Revelation", name);

            // Render a real chapter to confirm the reading path works end-to-end.
            let html = bible::render_chapter_html(genesis, &genesis.chapters[0]);
            assert!(html.contains("Genesis 1"), "{}: rendered HTML missing title", name);
        }
    }
}
