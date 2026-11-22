mod commands;
mod db;
mod server;

use db::Database;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Database>,
}

impl AppState {
    pub fn db(&self) -> Result<std::sync::MutexGuard<'_, Database>, db::DbError> {
        self.db
            .lock()
            .map_err(|_| db::DbError::Internal("database lock poisoned".into()))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Database::open(dir.join("inventory.db"))?;
            app.manage(AppState { db: Mutex::new(db) });
            app.manage(server::Pairing::default());

            // dev builds only: print a pairing code so we can test before the UI exists
            #[cfg(debug_assertions)]
            {
                let code = app.state::<server::Pairing>().start()?;
                println!("Pairing code (dev): {code}");
            }

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = server::run(handle).await {
                    eprintln!("Sync server stopped: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_products,
            commands::create_product,
            commands::delete_product,
            commands::update_product,
            commands::create_receipt,
            commands::add_item,
            commands::list_items,
            commands::receipt_totals,
            commands::finalize_receipt,
            commands::list_receipts,
            commands::list_products_with_stock,
            commands::list_pending_flags,
            commands::resolve_flag,
            commands::day_preview,
            commands::close_day,
            commands::start_pairing,
            commands::list_devices,
            commands::revoke_device,
            commands::list_pending_conflicts,
            commands::resolve_conflict
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
