pub mod db;
pub mod bible;
mod epub_parser;
mod embedded_library;
mod commands;

use tauri::Manager;

use crate::commands::AppState;
use crate::db::Db;

/// Run the application: open the database in the app data dir and start Tauri.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("spiritual_compass.db");
            let db = Db::new(db_path.to_str().unwrap())?;
            // First launch: seed the full embedded library so a fresh copy of the exe
            // already contains every book (no import step for the end user).
            if let Err(e) = commands::seed_library_if_empty(&db, &data_dir) {
                eprintln!("could not seed embedded library: {}", e);
            }
            app.manage(AppState {
                db,
                data_dir,
                bibles: std::sync::Mutex::new(std::collections::HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_books,
            commands::search_books,
            commands::get_book,
            commands::delete_book,
            commands::update_book_category,
            commands::import_epubs,
            commands::get_chapters,
            commands::get_bible_toc,
            commands::get_chapter_html,
            commands::save_progress,
            commands::get_progress,
            commands::add_bookmark,
            commands::list_bookmarks,
            commands::delete_bookmark,
            commands::get_setting,
            commands::set_setting
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
