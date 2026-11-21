//! EPUB parsing: metadata, cover image, spine chapters, and chapter HTML
//! with embedded assets inlined as data URIs (so the webview needs no asset
//! protocol configuration).

use base64::{engine::general_purpose::STANDARD, Engine as _};
use regex::Regex;
use std::io::Read;
use zip::ZipArchive;

#[derive(Clone)]
pub struct Chapter {
    pub href: String,
    pub title: String,
}

pub struct ParsedEpub {
    pub title: String,
    pub author: String,
    /// Raw image bytes of the cover (original encoding preserved).
    pub cover_bytes: Option<Vec<u8>>,
    /// Cover file extension hint from the source entry (e.g. "jpg").
    pub cover_ext: Option<String>,
    pub chapters: Vec<Chapter>,
}

struct ManifestItem {
    id: String,
    href: String,
    media_type: String,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Parse an .epub file fully (metadata + cover + chapter list).
pub fn parse_epub(path: &str) -> Result<ParsedEpub, String> {
    let mut zip = open_zip(path)?;
    let opf_path = find_opf_path(&mut zip)?;
    let opf_bytes = read_entry(&mut zip, &opf_path)
        .ok_or_else(|| "could not read OPF document from archive".to_string())?;

    let (title, author, items, spine_ids, cover_href) = parse_opf(&opf_bytes)?;

    // Cover image bytes. The href is relative to the OPF's directory, so resolve it;
    // fall back to the raw href in case a book stored it at the archive root.
    let mut cover_bytes = None;
    let mut cover_ext = None;
    if let Some(href) = &cover_href {
        let base_dir = dir_of(&opf_path);
        let candidates = [resolve_href(&base_dir, href), href.clone()];
        for cand in &candidates {
            if let Some(bytes) = read_entry(&mut zip, cand) {
                cover_bytes = Some(bytes);
                cover_ext = ext_of(cand);
                break;
            }
        }
    }

    // Chapters in spine order (HTML-ish items only). Spine hrefs are relative to the
    // OPF's directory, so resolve them to real archive paths.
    let opf_dir = dir_of(&opf_path);
    let mut chapters = Vec::new();
    for id in &spine_ids {
        if let Some(item) = items.iter().find(|i| i.id == *id) {
            if is_html_media(&item.media_type) || item.href.ends_with(".xhtml") || item.href.ends_with(".html") {
                let full_href = resolve_href(&opf_dir, &item.href);
                chapters.push(Chapter {
                    title: chapter_title_from_zip(&mut zip, &full_href).unwrap_or_else(|| "Untitled".into()),
                    href: full_href,
                });
            }
        }
    }

    Ok(ParsedEpub {
        title,
        author,
        cover_bytes,
        cover_ext,
        chapters,
    })
}

/// Read one chapter's HTML with all local images inlined as data URIs.
pub fn read_chapter_html(path: &str, href: &str) -> Result<String, String> {
    let mut zip = open_zip(path)?;
    let html = read_entry(&mut zip, href).ok_or_else(|| "chapter not found in archive".to_string())?;
    let text = String::from_utf8_lossy(&html);
    let base_dir = dir_of(href);
    Ok(inline_assets(&text, &base_dir, &mut zip))
}

/// Read every chapter as plain text in one pass (for indexing / full-text search).
pub fn all_chapter_texts(path: &str, chapters: &[Chapter]) -> Result<Vec<String>, String> {
    let mut zip = open_zip(path)?;
    let mut out = Vec::with_capacity(chapters.len());
    for ch in chapters {
        match read_entry(&mut zip, &ch.href) {
            Some(bytes) => out.push(strip_to_text(&String::from_utf8_lossy(&bytes))),
            None => out.push(String::new()),
        }
    }
    Ok(out)
}

/// Strip scripts/styles/tags and collapse whitespace into searchable plain text.
fn strip_to_text(html: &str) -> String {
    // Rust's `regex` crate has no backreferences, so use one pattern per tag type.
    let script_re = Regex::new(r#"(?is)<script\b[^>]*>.*?</script>"#).unwrap();
    let html = script_re.replace_all(html, " ");
    let style_re = Regex::new(r#"(?is)<style\b[^>]*>.*?</style>"#).unwrap();
    let html = style_re.replace_all(&html, " ");
    let no_tags = Regex::new(r#"<[^>]+>"#).unwrap();
    let text = no_tags.replace_all(&html, " ");
    let text = text
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let ws = Regex::new(r"\s+").unwrap();
    ws.replace_all(&text, " ").trim().to_string()
}

// ---------------------------------------------------------------------------
// Zip helpers
// ---------------------------------------------------------------------------

fn open_zip(path: &str) -> Result<ZipArchive<std::fs::File>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("cannot open {}: {}", path, e))?;
    zip::ZipArchive::new(file).map_err(|e| format!("not a valid EPUB archive: {}", e))
}

/// Try several common spellings of an entry name.
fn read_entry<'a>(zip: &mut ZipArchive<std::fs::File>, name: &'a str) -> Option<Vec<u8>> {
    let candidates = [name.to_string(), format!("/{}", name), percent_decode(name)];
    for cand in &candidates {
        if let Ok(mut f) = zip.by_name(cand) {
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).is_ok() {
                return Some(buf);
            }
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() && is_hex(bytes[i + 1]) && is_hex(bytes[i + 2]) {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'/'));
            i += 3;
        } else if bytes[i] == b'+' {
            out.push(b'/'); // EPUB hrefs sometimes use '+' for '/' — keep it simple
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_hex(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F')
}

/// Find the OPF path declared in META-INF/container.xml.
/// Standard EPUB form: `<rootfile full-path="OEBPS/content.opf" .../>`.
fn find_opf_path(zip: &mut ZipArchive<std::fs::File>) -> Result<String, String> {
    let container = read_entry(zip, "META-INF/container.xml")
        .ok_or_else(|| "missing META-INF/container.xml".to_string())?;
    let text = String::from_utf8_lossy(&container);
    // Match the full-path attribute on <rootfile>, tolerating other attributes and either quote style.
    let re = Regex::new(r#"<rootfile\b[^>]*\bfull-path\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap();
    if let Some(c) = re.captures(&text) {
        if let Some(m) = c.get(1).or_else(|| c.get(2)) {
            return Ok(m.as_str().trim().to_string());
        }
    }
    Err("no rootfile full-path in container.xml".to_string())
}

// ---------------------------------------------------------------------------
// OPF parsing (quick-xml event stream, matched by local element name so any
// namespace prefix works)
// ---------------------------------------------------------------------------

fn parse_opf(xml: &[u8]) -> Result<(String, String, Vec<ManifestItem>, Vec<String>, Option<String>), String> {
    let text = String::from_utf8_lossy(xml);

    // <dc:title>/<dc:creator> — any namespace prefix.
    let title_re = Regex::new(r#"<(?:\w+:)?title[^>]*>(.*?)</(?:\w+:)?title>"#).unwrap();
    let author_re = Regex::new(r#"<(?:\w+:)?creator[^>]*>(.*?)</(?:\w+:)?creator>"#).unwrap();
    let mut title = String::new();
    if let Some(c) = title_re.captures(&text) { title = strip_tags(&c[1]).to_string(); }
    let mut author = String::new();
    if let Some(c) = author_re.captures(&text) { author = strip_tags(&c[1]).to_string(); }

    // <item> entries in the manifest.
    let item_re = Regex::new(r#"<item\b([^>]*)/?>"#).unwrap();
    let mut items: Vec<ManifestItem> = Vec::new();
    let mut cover_item_id: Option<String> = None;
    for caps in item_re.captures_iter(&text) {
        let attrs = &caps[1];
        let id = attr(attrs, "id");
        let href = attr(attrs, "href");
        let media_type = attr(attrs, "media-type");
        // EPUB 3 marks the cover with properties="cover-image".
        if attr(attrs, "properties").contains("cover-image") {
            cover_item_id = Some(id.clone());
        }
        items.push(ManifestItem { id, href, media_type });
    }

    // Cover: prefer EPUB 3's properties marker; fall back to EPUB 2's
    // <meta name="cover" content="<item-id>"/> reference.
    let mut cover_href = cover_item_id.and_then(|cid| items.iter().find(|i| i.id == cid)).map(|i| i.href.clone());
    if cover_href.is_none() {
        let meta_re = Regex::new(r#"<meta\b[^>]*\bname\s*=\s*\"?cover\"?[^>]*\bcontent\s*=\s*\"([^\"]*)\""#).unwrap();
        if let Some(c) = meta_re.captures(&text) {
            let cover_id = c[1].trim().to_string();
            if let Some(item) = items.iter().find(|i| i.id == cover_id) {
                cover_href = Some(item.href.clone());
            }
        }
    }

    // <itemref> entries in the spine (in document order).
    let itemref_re = Regex::new(r#"<itemref\b([^>]*)/?>"#).unwrap();
    let mut spine_ids: Vec<String> = Vec::new();
    for caps in itemref_re.captures_iter(&text) {
        let idref = attr(&caps[1], "idref");
        if !idref.is_empty() { spine_ids.push(idref); }
    }

    Ok((title, author, items, spine_ids, cover_href))
}

/// Extract a single attribute value from an XML tag's attribute string.
fn attr(attrs: &str, name: &str) -> String {
    let re = Regex::new(&format!(r#"(?:^|\s){name}\s*=\s*(?:\"([^\"]*)\"|'([^']*)')"#)).unwrap();
    match re.captures(attrs) {
        Some(c) => c.get(1).or_else(|| c.get(2)).map(|m| m.as_str().to_string()).unwrap_or_default(),
        None => String::new(),
    }
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(s, "").trim().to_string()
}

fn is_html_media(media_type: &str) -> bool {
    media_type.contains("html") || media_type.contains("xhtml")
}

fn ext_of(name: &str) -> Option<String> {
    name.rsplit('.').next().and_then(|e| {
        let e = e.to_lowercase();
        if matches!(e.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") { Some(e) } else { None }
    })
}

fn dir_of(href: &str) -> String {
    match href.rfind('/') {
        Some(i) => href[..i].to_string(),
        None => String::new(),
    }
}

/// Resolve a (possibly relative) asset href against the chapter's directory.
fn resolve_href(base_dir: &str, href: &str) -> String {
    if href.starts_with("http:") || href.starts_with("https:") || href.starts_with("data:") {
        return href.to_string();
    }
    let clean = href.trim_start_matches('/');
    if base_dir.is_empty() {
        clean.to_string()
    } else {
        // Naive join then collapse ".." segments.
        let mut parts: Vec<&str> = base_dir.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
        for seg in clean.split('/') {
            if seg == "." { continue; }
            // Never panic on a malformed "../" that walks above the archive root.
            if seg == ".." { let _ = parts.pop(); } else if !seg.is_empty() { parts.push(seg); }
        }
        parts.join("/")
    }
}

fn mime_for(name: &str) -> &'static str {
    match ext_of(name).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    }
}

/// Replace local image srcs with inlined data URIs.
fn inline_assets(html: &str, base_dir: &str, zip: &mut ZipArchive<std::fs::File>) -> String {
    let re = Regex::new(r#"src\s*=\s*"([^"]*)""#).unwrap();
    re.replace_all(html, |caps: &regex::Captures| {
        let src = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if src.is_empty() || src.starts_with("data:") || src.starts_with("http:") || src.starts_with("https:") {
            return format!("src=\"{}\"", src);
        }
        let resolved = resolve_href(base_dir, src);
        match read_entry(zip, &resolved) {
            Some(bytes) => {
                let b64 = STANDARD.encode(&bytes);
                format!("src=\"data:{};base64,{}\"", mime_for(&resolved), b64)
            }
            None => format!("src=\"{}\"", src),
        }
    }).to_string()
}

fn chapter_title_from_zip(zip: &mut ZipArchive<std::fs::File>, href: &str) -> Option<String> {
    let bytes = read_entry(zip, href)?;
    let text = String::from_utf8_lossy(&bytes);
    let re = Regex::new(r#"<h[1-6][^>]*>(.*?)</h[1-6]>"#).unwrap();
    let caps = re.captures_iter(&text).next()?;
    let title = strip_tags(&caps[1]).trim().to_string();
    if title.is_empty() { None } else { Some(title) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test against a real EPUB in the app data dir. We don't assert an exact
    /// title/author because book_0.epub is whatever the user imported — we only assert
    /// that parsing succeeds and yields coherent, non-empty structure.
    #[test]
    fn parse_real_epub() {
        let path = r"C:\Users\Spud\AppData\Local\com.spud.spiritualcompass\books\book_0.epub";
        if !std::path::Path::new(path).exists() {
            eprintln!("skipping: no test epub at {}", path);
            return;
        }
        let parsed = parse_epub(path).expect("parse should succeed");
        assert!(!parsed.title.trim().is_empty(), "title empty");
        assert!(parsed.chapters.len() > 0, "no chapters found");
        // First chapter must be readable.
        let html = read_chapter_html(path, &parsed.chapters[0].href).expect("chapter 0 should read");
        assert!(!html.trim().is_empty(), "chapter 0 empty");
    }

    /// Regression: strip_to_text's regexes must compile (Rust `regex` has no backrefs)
    /// and must drop script/style content while keeping visible text.
    #[test]
    fn strips_scripts_and_tags() {
        let html = "<html><head><style>body{}</style></head><body><script>x=1</script><p>Hello <b>World</b></p></body></html>";
        let text = strip_to_text(html);
        assert!(!text.contains("x=1"), "script content should be stripped");
        assert!(text.contains("Hello World"), "visible text missing: {}", text);
    }
}
