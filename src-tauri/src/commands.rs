use crate::db::Product;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn list_products(state: State<'_, AppState>) -> Result<Vec<Product>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_products().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_product(
    state: State<'_, AppState>,
    name: String,
    size: Option<String>,
    notes: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.create_product(&name, size.as_deref(), notes.as_deref())
        .map_err(|e| e.to_string())
}