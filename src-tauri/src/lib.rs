mod commands;
mod db;
mod domain;
mod game;
mod ocr;
mod providers;
mod recommendation;

use rusqlite::Connection;
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{path::BaseDirectory, Emitter, Manager};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub ocr: Mutex<ocr::OcrClient>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["ctrl+shift+f6", "ctrl+shift+f7", "ctrl+shift+f8"])
                .expect("valid global shortcuts")
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let key = shortcut.to_string().to_ascii_lowercase();
                    let event_name = if key.ends_with("f6") {
                        "ocr:request"
                    } else if key.ends_with("f7") {
                        "game:refresh"
                    } else if key.ends_with("f8") {
                        "ocr:hide"
                    } else {
                        return;
                    };
                    let _ = app.emit_to("main", event_name, ());
                })
                .build(),
        )
        .setup(|app| {
            let app_data_dir = app.path().app_local_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let db_path: PathBuf = app_data_dir.join("aram-companion.sqlite3");
            let mut connection = Connection::open(db_path)?;
            connection.execute_batch(include_str!("../data/migrations/0001_initial.sql"))?;
            connection.pragma_update(None, "foreign_keys", "ON")?;
            let augment_count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM champion_augment_grades",
                [],
                |row| row.get(0),
            )?;
            if augment_count == 0 {
                db::snapshot::import_augment_grades_json(
                    &mut connection,
                    include_str!("../../data/seed/augment_grades.json"),
                )
                .map_err(std::io::Error::other)?;
            }
            let resource_executable = app
                .path()
                .resolve(
                    "workers/ocr/dist/aram-ocr/aram-ocr.exe",
                    BaseDirectory::Resource,
                )
                .ok();
            let resource_layout = app
                .path()
                .resolve("workers/ocr/card-layout.json", BaseDirectory::Resource)
                .ok();
            let development_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("workers")
                .join("ocr");
            app.manage(AppState {
                db: Mutex::new(connection),
                ocr: Mutex::new(ocr::OcrClient::new(
                    resource_executable,
                    resource_layout,
                    development_dir,
                )),
            });
            providers::opgg::start_rank_sync(app.handle().clone());
            ocr::start_automatic_detection(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status::get_app_status,
            commands::rankings::list_hero_rankings,
            commands::champions::get_hero_detail,
            commands::champions::get_augment_grades,
            providers::opgg::refresh_champion_augments,
            commands::builds::recommend_build,
            commands::providers::import_rankings_json,
            commands::providers::import_augment_grades_json,
            commands::ai::get_ai_settings,
            commands::ai::save_ai_settings,
            commands::game::get_game_detection_settings,
            commands::game::save_game_detection_settings,
            commands::game::show_champion_rank_overlay,
            commands::game::hide_champion_rank_overlay,
            game::watcher::get_game_state,
            commands::ocr::recognize_current_screen,
            commands::ocr::show_ocr_overlay,
            commands::ocr::hide_ocr_overlay
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ARAM Companion");
}
