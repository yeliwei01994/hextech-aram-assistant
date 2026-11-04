mod commands;
mod db;
mod domain;
mod game;
mod ocr;
mod providers;
mod recommendation;

use rusqlite::Connection;
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    path::BaseDirectory,
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewWindowBuilder,
};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub ocr: Mutex<ocr::OcrClient>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
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
            use tauri_plugin_global_shortcut::GlobalShortcutExt;

            for shortcut in ["ctrl+shift+f6", "ctrl+shift+f7", "ctrl+shift+f8"] {
                if let Err(error) = app.global_shortcut().register(shortcut) {
                    eprintln!("注册全局快捷键 {shortcut} 失败，应用继续运行：{error}");
                }
            }

            let app_data_dir = app.path().app_local_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let db_path: PathBuf = app_data_dir.join("aram-companion.sqlite3");
            let connection = Connection::open(db_path)?;
            connection.execute_batch(include_str!("../data/migrations/0001_initial.sql"))?;
            connection.pragma_update(None, "foreign_keys", "ON")?;
            providers::aramhex::migrate(&connection)?;
            connection.execute(
                "INSERT OR IGNORE INTO app_settings (key, value) VALUES (?1, 'true')",
                ["game_detection_enabled"],
            )?;
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
            // Register shared state before importing the larger seed snapshot.
            // The webview can begin issuing commands as soon as it loads, so the
            // state must be available before any potentially slow first-run work.
            {
                let state = app.state::<AppState>();
                let mut db = state
                    .db
                    .lock()
                    .map_err(|_| std::io::Error::other("本地数据库锁定失败"))?;
                let augment_count: i64 = db.query_row(
                    "SELECT COUNT(*) FROM champion_augment_grades",
                    [],
                    |row| row.get(0),
                )?;
                if augment_count == 0 {
                    db::snapshot::import_augment_grades_json(
                        &mut db,
                        include_str!("../../data/seed/augment_grades.json"),
                    )
                    .map_err(std::io::Error::other)?;
                }
                providers::aramhex::seed(&mut db).map_err(std::io::Error::other)?;
            }
            // Tauri otherwise creates configured webviews before this setup
            // callback. Create them only after AppState is registered so the
            // frontend cannot invoke state-backed commands too early.
            for window_config in &app.config().app.windows {
                WebviewWindowBuilder::from_config(app.handle(), window_config)?.build()?;
            }
            providers::aramhex::start_sync(app.handle().clone());
            ocr::start_automatic_detection(app.handle().clone());
            game::watcher::start_game_state_monitor(app.handle().clone());

            let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;
            if let Some(icon) = app.default_window_icon() {
                TrayIconBuilder::new()
                    .icon(icon.clone())
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .build(app)?;
            } else {
                eprintln!("无法创建系统托盘：默认应用图标不存在");
            }

            let main = app.get_webview_window("main").expect("main window exists");
            let close_window = main.clone();
            main.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = close_window.hide();
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status::get_app_status,
            commands::rankings::list_hero_rankings,
            commands::champions::get_hero_detail,
            commands::champions::get_augment_grades,
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
            commands::notifications::get_notification_permission,
            commands::notifications::request_notification_permission,
            commands::notifications::send_app_notification,
            commands::ocr::recognize_current_screen,
            commands::ocr::show_ocr_overlay,
            commands::ocr::hide_ocr_overlay
        ])
        .run(tauri::generate_context!())
        .expect("failed to run 海克斯大乱斗小助手");
}
