//! Bible normalization: turn the many ways a whole Bible gets shipped in one EPUB
//! into one canonical model (book -> chapter -> verses) so every translation reads
//! with proper book/chapter/verse structure and can be navigated chapter by chapter.
//!
//! Formats handled (all found in real user libraries):
//!   * escaped-XML  — `<book number="1">` / `<chapter number="1">` / `<verse number="1">…</verse>`
//!                    pasted into HTML paragraph soup (CSB, NKJV, NASB, NRSV, RSV, LSB).
//!   * calibre      — a stream of `Old|New <book> <chapter> <verse> text` tokens
//!                    (NIV and other Calibre exports), often one giant line.
//!   * inline       — paragraphs beginning with an `N:M` verse marker, one file per book
//!                    (KJV). Book name comes from its canonical index.
//!
//! The canonical 66-book Protestant canon labels books by index; chapter and verse
//! numbers always come from the source itself so each translation reads as-is.

use regex::Regex;

/// One verse of scripture.
#[derive(Clone)]
pub struct Verse {
    pub number: u32,
    pub text: String,
}

/// One chapter (e.g. "Exodus 3").
#[derive(Clone)]
pub struct Chapter {
    pub book_no: u32,
    pub chapter_no: u32,
    pub verses: Vec<Verse>,
}

/// One book of the Bible with its chapters in order.
#[derive(Clone)]
pub struct Book {
    pub no: u32,
    pub name: String,
    pub chapters: Vec<Chapter>,
}

impl Book {
    /// "Exodus 3" — the label shown for a chapter page.
    pub fn chapter_title(&self, ch: &Chapter) -> String {
        format!("{} {}", self.name, ch.chapter_no)
    }
}

/// Canonical Protestant canon (index 1..=66). Names are what we show; chapter/verse
/// numbers always come from the source file so each translation reads as-is.
pub const CANON: &[&str] = &[
    "Genesis", "Exodus", "Leviticus", "Numbers", "Deuteronomy", "Joshua", "Judges", "Ruth",
    "1 Samuel", "2 Samuel", "1 Kings", "2 Kings", "1 Chronicles", "2 Chronicles", "Ezra",
    "Nehemiah", "Esther", "Job", "Psalms", "Proverbs", "Ecclesiastes", "Song of Solomon",
    "Isaiah", "Jeremiah", "Lamentations", "Ezekiel", "Daniel", "Hosea", "Joel", "Amos",
    "Obadiah", "Jonah", "Micah", "Nahum", "Habakkuk", "Zephaniah", "Haggai", "Zechariah",
    "Malachi", "Matthew", "Mark", "Luke", "John", "Acts", "Romans", "1 Corinthians",
    "2 Corinthians", "Galatians", "Ephesians", "Philippians", "Colossians", "1 Thessalonians",
    "2 Thessalonians", "1 Timothy", "2 Timothy", "Titus", "Philemon", "Hebrews", "James",
    "1 Peter", "2 Peter", "1 John", "2 John", "3 John", "Jude", "Revelation",
];

/// Name for a 1-based book index, or None if out of canon range.
pub fn book_name(no: u32) -> Option<&'static str> {
    CANON.get((no as usize).saturating_sub(1)).copied()
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Normalize a whole-Bible document into canonical books. Returns None when the text is
/// not a recognized Bible layout, so callers can fall back to raw rendering. A genuine
/// whole Bible has many books; requiring >= 2 keeps a single-book file from being mistaken
/// for one (which matters when this is tried per spine item on KJV-style EPUBs).
///
/// Works directly on the raw HTML (matching escaped-XML markers or calibre tokens)
/// so no giant intermediate string is allocated — critical for 20 MB+ files.
pub fn normalize_whole_bible(html: &str) -> Option<Vec<Book>> {
    let books = parse_xml_raw(html).or_else(|| parse_calibre_raw(html))?;
    if books.len() < 2 { return None; }
    Some(validate_books(books))
}

/// Normalize one per-book file (a single book's text) into its chapters, using inline
/// `N:M` verse markers. Returns the parsed chapters; the caller assigns `no`/`name`.
pub fn parse_inline_book(html: &str) -> Option<Vec<Chapter>> {
    let p_re = Regex::new(r#"(?is)<p\b[^>]*>(.*?)</p>"#).unwrap();
    // Every N:M reference starts a verse — KJV packs several verses per paragraph.
    let ref_re = Regex::new(r"\b(\d+):(\d+)\b").unwrap();

    let mut chapters: Vec<Chapter> = Vec::new();
    let mut cur_ch: Option<u32> = None;
    let mut verses: Vec<Verse> = Vec::new();

    for cap in p_re.captures_iter(html) {
        let text = unescape(&strip_tags(&cap[1])).trim().to_string();
        if text.is_empty() { continue; }

        // Walk every verse reference; prose between them folds into the prior verse.
        let mut last_end = 0usize;
        for m in ref_re.captures_iter(&text) {
            let c: u32 = match m.get(1).unwrap().as_str().parse() { Ok(x) => x, Err(_) => continue };
            let v: u32 = match m.get(2).unwrap().as_str().parse() { Ok(x) => x, Err(_) => continue };
            let (s, e) = (m.get(0).unwrap().start(), m.get(0).unwrap().end());

            if s > last_end && cur_ch.is_some() {
                let gap = text[last_end..s].trim();
                if !gap.is_empty() {
                    if let Some(lv) = verses.last_mut() { lv.text.push_str(&format!(" {}", gap)); }
                }
            }

            if cur_ch != Some(c) { push_chapter(&mut chapters, &mut verses); }
            cur_ch = Some(c);
            verses.push(Verse { number: v, text: String::new() });
            last_end = e;
        }
        // Prose after the final marker (or a paragraph with no markers at all).
        if last_end < text.len() && cur_ch.is_some() {
            let tail = text[last_end..].trim();
            if !tail.is_empty() {
                if let Some(lv) = verses.last_mut() { lv.text.push_str(&format!(" {}", tail)); }
            }
        }
    }
    push_chapter(&mut chapters, &mut verses);

    if chapters.is_empty() || !chapters.iter().any(|c| !c.verses.is_empty()) { return None; }
    Some(chapters)
}

/// Render one chapter as clean reading HTML (a titled header + one `<p>` per verse).
pub fn render_chapter_html(book: &Book, ch: &Chapter) -> String {
    let mut out = String::new();
    out.push_str(&format!("<header class=\"bible-chapter-title\">{}</header>\n", book.chapter_title(ch)));
    for v in &ch.verses {
        out.push_str(&format!(
            "<p><span class=\"bible-vnum\" data-verse=\"{}\">{}</span> {}</p>\n",
            v.number, v.number, escape_html(&v.text)));
    }
    out
}

/// Plain text for one chapter (for full-text search indexing).
pub fn chapter_text(book: &Book, ch: &Chapter) -> String {
    let mut s = format!("{} {}", book.name, ch.chapter_no);
    for v in &ch.verses { s.push_str(&format!(" {}:{} {}", v.number, " ", v.text)); }
    s.trim().to_string()
}

// ---------------------------------------------------------------------------
// Whole-bible format parsers (scan raw HTML directly — no intermediate string)
// ---------------------------------------------------------------------------

/// escaped-XML in raw HTML: markers appear as `&lt;book number="1"&gt;` etc.
fn parse_xml_raw(html: &str) -> Option<Vec<Book>> {
    // Markers are HTML-escaped in the source (&lt; … &gt;); allow a literal form too.
    let book_re = Regex::new(r#"&lt;book\s+number="(\d+)""#).unwrap();
    let chap_re = Regex::new(r#"&lt;chapter\s+number="(\d+)""#).unwrap();
    let verse_re = Regex::new(r#"&lt;verse\s+number="(\d+)"(?:&gt;|>)(.*?)&lt;/verse&gt;"#).unwrap();

    // Marker positions, so each verse is attributed to the book/chapter opened before it.
    let mut books_pos: Vec<(usize, u32)> = Vec::new();
    for m in book_re.captures_iter(html) { if let Some(n) = first_num(&m[1]) { books_pos.push((m.get(0).unwrap().start(), n)); } }
    let mut chaps_pos: Vec<(usize, u32)> = Vec::new();
    for m in chap_re.captures_iter(html) { if let Some(n) = first_num(&m[1]) { chaps_pos.push((m.get(0).unwrap().start(), n)); } }

    let mut out_books: Vec<Book> = Vec::new();
    let (mut bi, mut ci) = (0usize, 0usize);
    let (mut cur_book, mut cur_chap): (u32, u32) = (0, 0);

    for m in verse_re.captures_iter(html) {
        let vpos = m.get(0).unwrap().start();
        while bi < books_pos.len() && books_pos[bi].0 <= vpos { cur_book = books_pos[bi].1; bi += 1; }
        while ci < chaps_pos.len() && chaps_pos[ci].0 <= vpos { cur_chap = chaps_pos[ci].1; ci += 1; }

        let n: u32 = m.get(1).unwrap().as_str().parse().ok()?;
        // Verse text may contain HTML entities / tags — clean just this small snippet.
        let raw_text = &m[2];
        let text = unescape(&strip_tags(raw_text)).trim().to_string();

        if out_books.last().map(|b| b.no != cur_book).unwrap_or(true) {
            let name = book_name(cur_book).unwrap_or("Unknown").to_string();
            out_books.push(Book { no: cur_book, name, chapters: Vec::new() });
        }
        let bk = out_books.last_mut().unwrap();
        if bk.chapters.last().map(|c| c.chapter_no != cur_chap).unwrap_or(true) {
            bk.chapters.push(Chapter { book_no: cur_book, chapter_no: cur_chap, verses: Vec::new() });
        }
        bk.chapters.last_mut().unwrap().verses.push(Verse { number: n, text });
    }

    if out_books.is_empty() || !out_books.iter().any(|b| !b.chapters.is_empty()) { return None; }
    Some(out_books)
}

/// calibre tokens in raw HTML: `Old 1 1 1 text Old 1 1 2 text …`
fn parse_calibre_raw(html: &str) -> Option<Vec<Book>> {
    let re = Regex::new(r"(?:Old|New)\s+(\d+)\s+(\d+)\s+(\d+)").unwrap();

    // (book, chapter, verse, match_start, text_start) in stream order.
    let mut toks: Vec<(u32, u32, u32, usize, usize)> = Vec::new();
    for m in re.captures_iter(html) {
        let b: u32 = match m.get(1).unwrap().as_str().parse() { Ok(x) => x, Err(_) => continue };
        let c: u32 = match m.get(2).unwrap().as_str().parse() { Ok(x) => x, Err(_) => continue };
        let v: u32 = match m.get(3).unwrap().as_str().parse() { Ok(x) => x, Err(_) => continue };
        let (s, e) = (m.get(0).unwrap().start(), m.get(0).unwrap().end());
        toks.push((b, c, v, s, e));
    }
    if toks.is_empty() { return None; }

    let mut books: Vec<Book> = Vec::new();
    for (i, &(b, c, v, _start, text_start)) in toks.iter().enumerate() {
        // Verse text runs to just before the next token's prefix.
        let end = toks.get(i + 1).map(|n| n.3).unwrap_or(html.len());
        let raw = &html[text_start..end];
        let text = unescape(&strip_tags(raw)).trim().to_string();

        if books.last().map(|bk| bk.no != b).unwrap_or(true) {
            let name = book_name(b).unwrap_or("Unknown").to_string();
            books.push(Book { no: b, name, chapters: Vec::new() });
        }
        let bk = books.last_mut().unwrap();
        if bk.chapters.last().map(|ch| ch.chapter_no != c).unwrap_or(true) {
            bk.chapters.push(Chapter { book_no: b, chapter_no: c, verses: Vec::new() });
        }
        bk.chapters.last_mut().unwrap().verses.push(Verse { number: v, text });
    }

    if books.is_empty() || !books.iter().any(|b| !b.chapters.is_empty()) { return None; }
    Some(books)
}

/// First integer found in a marker string.
fn first_num(s: &str) -> Option<u32> {
    let re = Regex::new(r"(\d+)").unwrap();
    re.find(s).and_then(|m| m.as_str().parse::<u32>().ok())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Push the pending verses as a new chapter under the last book (creating one if needed).
fn push_chapter(chapters: &mut Vec<Chapter>, verses: &mut Vec<Verse>) {
    if verses.is_empty() { return; }
    let n = chapters.len() + 1;
    chapters.push(Chapter { book_no: 0, chapter_no: n as u32, verses: std::mem::take(verses) });
}

/// Re-label books by canonical position when the count matches the canon (so a
/// translation that omits/renumbers books still maps correctly); otherwise keep source.
fn validate_books(mut books: Vec<Book>) -> Vec<Book> {
    if books.len() == CANON.len() {
        for (i, b) in books.iter_mut().enumerate() {
            let no = (i + 1) as u32;
            b.no = no;
            if let Some(name) = book_name(no) { b.name = name.to_string(); }
            for ch in b.chapters.iter_mut() { ch.book_no = no; }
        }
    }
    books
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(s, "").to_string()
}

/// Decode the entities we care about (the source is HTML-escaped XML).
fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
     .replace("&gt;", ">")
     .replace("&quot;", "\"")
     .replace("&#39;", "'")
     .replace("&amp;", "&")
}

/// Minimal HTML escaping for verse text we re-emit (guards against stray markup).
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_escaped_xml_bible() {
        let html = "<body>\n\
            <p>&lt;book number=\"1\"&gt;</p>\n\
            <p>&lt;chapter number=\"1\"&gt;</p>\n\
            <p>&lt;verse number=\"1\"&gt;In the beginning God created.&lt;/verse&gt;</p>\n\
            <p>&lt;verse number=\"2\"&gt;And He saw it was good.&lt;/verse&gt;</p>\n\
            <p>&lt;book number=\"2\"&gt;</p>\n\
            <p>&lt;chapter number=\"1\"&gt;</p>\n\
            <p>&lt;verse number=\"1\"&gt;These are the names.&lt;/verse&gt;</p>\n\
            </body>";
        let books = normalize_whole_bible(html).expect("should parse");
        assert_eq!(books.len(), 2);
        assert_eq!(books[0].name, "Genesis");
        assert_eq!(books[1].name, "Exodus");
        assert_eq!(books[0].chapters[0].verses.len(), 2);
        assert!(books[0].chapters[0].verses[0].text.contains("beginning"));
    }

    #[test]
    fn parses_calibre_bible_single_line() {
        // NIV-style: one giant line, tokens separated by spaces. Two books so the
        // whole-Bible guard (>= 2) is satisfied and book/chapter transitions are hit.
        let html = "<body><p>Old 1 1 1 In the beginning God created. Old 1 1 2 And He saw it was good. \
                    Old 1 2 1 The heavens were finished. Old 2 1 1 These are the names.</p></body>";
        let books = normalize_whole_bible(html).expect("should parse");
        assert_eq!(books.len(), 2);
        assert_eq!(books[0].name, "Genesis");
        assert_eq!(books[1].name, "Exodus");
        assert_eq!(books[0].chapters.len(), 2);
        assert_eq!(books[0].chapters[0].verses.len(), 2);
        assert_eq!(books[0].chapters[1].chapter_no, 2);
    }

    #[test]
    fn single_book_file_is_not_a_whole_bible() {
        // A lone book (e.g. one KJV spine item) must NOT be treated as a whole Bible.
        let html = "<body><p>Old 1 1 1 In the beginning God created.</p></body>";
        assert!(normalize_whole_bible(html).is_none());
    }

    #[test]
    fn parses_inline_book_file() {
        let html = "<body>\n\
            <p>1:1 In the beginning God created.</p>\n\
            <p>1:2 And He saw it was good.</p>\n\
            <p>2:1 The heavens and the earth were finished.</p>\n\
            </body>";
        let chapters = parse_inline_book(html).expect("should parse");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].verses.len(), 2);
    }

    #[test]
    fn rejects_non_bible_text() {
        let html = "<body><p>Just a novel paragraph with no verse structure at all.</p></body>";
        assert!(normalize_whole_bible(html).is_none());
        assert!(parse_inline_book(html).is_none());
    }

    #[test]
    fn renders_chapter_html() {
        let book = Book {
            no: 2, name: "Exodus".into(),
            chapters: vec![Chapter {
                book_no: 2, chapter_no: 3,
                verses: vec![Verse { number: 1, text: "And the LORD spoke unto Moses.".into() }],
            }],
        };
        let html = render_chapter_html(&book, &book.chapters[0]);
        assert!(html.contains("Exodus 3"));
        assert!(html.contains("<span class=\"bible-vnum\">1</span>"));
    }
}
