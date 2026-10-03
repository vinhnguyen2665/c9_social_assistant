pub mod commands;
pub mod crawler;
pub mod db;
pub mod hwid;
pub mod license;
pub mod tracker;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

use commands::AppState;
use crawler::FacebookCrawler;
use db::Database;
use license::{start_heartbeat_worker, LicenseManager};
use tracker::{start_watchlist_worker, WatchlistTracker};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Determine SQLite database storage path
            let db_path = match app.path().app_data_dir() {
                Ok(mut p) => {
                    let _ = std::fs::create_dir_all(&p);
                    p.push("c9_social_assistant.db");
                    p
                }
                Err(_) => PathBuf::from("./c9_social_assistant.db"),
            };

            let db = Arc::new(Database::new(db_path).expect("Failed to initialize SQLite Database"));
            let crawler = Arc::new(FacebookCrawler::new());
            let license = Arc::new(LicenseManager::new());
            let tracker = Arc::new(WatchlistTracker::new(db.clone(), crawler.clone(), license.clone()));

            // Restore saved Facebook session if available
            if let Ok(Some(saved_session_json)) = db.get_setting("fb_session") {
                if let Ok(saved_session) = serde_json::from_str::<crawler::FacebookSession>(&saved_session_json) {
                    let crawler_clone = crawler.clone();
                    tauri::async_runtime::spawn(async move {
                        crawler_clone.set_session(saved_session).await;
                    });
                }
            }

            // Manage State in Tauri
            app.manage(AppState {
                db: db.clone(),
                crawler: crawler.clone(),
                license: license.clone(),
                tracker: tracker.clone(),
            });

            // Start background Heartbeat Worker (5 min cycle, strict online-only rule)
            start_heartbeat_worker(app.handle().clone(), license.clone());

            // Start background Watchlist Incremental Crawling Worker
            start_watchlist_worker(app.handle().clone(), tracker.clone(), db.clone(), license.clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::login_license,
            commands::get_license_status,
            commands::add_monitored_post,
            commands::get_monitored_posts,
            commands::toggle_post_status,
            commands::update_monitored_post,
            commands::delete_monitored_post,
            commands::scan_post_now,
            commands::get_post_comments,
            commands::set_facebook_session,
            commands::get_facebook_session_status,
            commands::open_facebook_login_window,
            commands::extract_facebook_session_from_window,
            commands::sync_all_to_cloud,
            commands::seed_demo_data
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
