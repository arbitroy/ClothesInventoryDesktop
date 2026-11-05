mod commands;
mod db;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
