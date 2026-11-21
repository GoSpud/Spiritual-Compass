# Spiritual Compass

A calm, focused e-book reader and library for Windows — built with **Rust + Tauri 2** on the backend and **React + TypeScript + Tailwind CSS v4** in the webview. Fully offline: no cloud, no telemetry.

![status](https://img.shields/placeholder) *v0.1.0*

## Features

- **Import `.epub` books** (multi-select) — metadata and cover extracted automatically
- **Four categories**: EGW Library · Devotionals · Biographies · Bibles, with search
- **Reader** with chapter navigation, font size (A−/A+), line spacing, and per-book images inlined from the archive
- **Reading progress** — autosaves every ~5 s and on close; restores your exact position on relaunch; shows % on library cards
- **Bookmarks** — add at current position, jump back instantly, delete
- **Three themes** — Light · Dark (charcoal) · Reading (warm sepia), persisted across restarts

## Data model (SQLite in the app data dir)

| Table              | Purpose                                             |
| ------------------ | --------------------------------------------------- |
| `books`            | title, author, category, file/cover paths           |
| `reading_progress` | chapter index + scroll offset + percent per book    |
| `bookmarks`        | saved positions per book                            |
| `settings`         | theme, font size, line height                       |

## Development

Prerequisites: Rust (stable), Node 18+, and the **MSVC C++ build tools** (Visual Studio Build Tools with the "Desktop development with C++" workload) — required to link Tauri on Windows.

```powershell
# install frontend deps
npm install

# run in dev mode (Vite HMR + Rust rebuilds)
cargo tauri dev
```

## Production build (.exe installer)

```powershell
cargo tauri build
```

Outputs land in `src-tauri/target/release/bundle/`:

- **NSIS** — `Spiritual Compass Setup.x86_...` (primary deliverable; installs to Program Files, adds a Start Menu entry and desktop shortcut)
- **MSI** — machine-wide installer alternative

The installed app is fully self-contained: no Node or Rust toolchain needed at runtime.

## Project layout

```
├─ index.html               # webview entry
├─ src/                     # React frontend
│  ├─ App.tsx               # Library ⇄ Reader routing
│  ├─ theme/ThemeProvider   # persisted Light/Dark/Reading themes
│  ├─ styles/index.css      # design tokens (CSS custom properties)
│  └─ components/
│     ├─ Library/           # sidebar, book grid, import flow
│     └─ Reader/            # chapter pane, toolbar, bookmarks drawer
├─ src-tauri/               # Rust backend
│  ├─ tauri.conf.json       # window + NSIS/MSI bundling config
│  └─ src/
│     ├─ main.rs / lib.rs   # app entry, DB state registration
│     ├─ db.rs              # SQLite schema (CREATE TABLE IF NOT EXISTS)
│     ├─ epub_parser.rs     # zip + quick-xml EPUB parsing, asset inlining
│     └─ commands.rs        # Tauri commands: books/import/reader/progress/bookmarks/settings
└─ scripts/generate-icons.mjs  # dependency-free PNG/ICO compass icon generator
```

## Notes & limitations

- EPUBs are parsed directly (`zip` + `quick-xml`); DRM-protected files are not supported.
- Cover images and chapter assets are served from the app data dir; covers use Tauri's asset protocol, in-chapter images are inlined as data URIs by Rust.
- Category names are fixed: `EGW Library`, `Devotionals`, `Biographies`, `Bibles`.
