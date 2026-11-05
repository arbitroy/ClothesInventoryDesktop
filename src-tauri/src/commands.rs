use crate::AppState;
use crate::db::{ContainerItem, DbError, Product, Receipt, ReceiptTotals};
use tauri::State;

#[tauri::command]
pub fn list_products(state: State<'_, AppState>) -> Result<Vec<Product>, DbError> {
    let db = state.db()?;
    db.list_products()
}

#[tauri::command]
pub fn create_product(
    state: State<'_, AppState>,
    name: String,
    size: Option<String>,
    notes: Option<String>,
) -> Result<String, DbError> {
    let db = state.db()?;
    db.create_product(&name, size.as_deref(), notes.as_deref())
}

#[tauri::command]
pub fn delete_product(state: State<'_, AppState>, id: String) -> Result<usize, DbError> {
    let db = state.db()?;
    db.delete_product(&id)
}

#[tauri::command]
pub fn update_product(state: State<'_, AppState>, product: Product) -> Result<usize, DbError> {
    let db = state.db()?;
    db.update_product(&product)
}

#[tauri::command]
pub fn create_receipt(
    state: State<'_, AppState>,
    container_number: &str,
    received_date: i64,
    notes: Option<&str>,
) -> Result<String, DbError> {
    let db = state.db()?;
    db.create_receipt(container_number, received_date, notes)
}

#[tauri::command]
pub fn add_item(
    state: State<'_, AppState>,
    receipt_id: &str,
    product_id: &str,
    quantity: i64,
    notes: Option<&str>,
) -> Result<String, DbError> {
    let db = state.db()?;
    db.add_item(receipt_id, product_id, quantity, notes)
}

#[tauri::command]
pub fn list_items(
    state: State<'_, AppState>,
    receipt_id: &str,
) -> Result<Vec<ContainerItem>, DbError> {
    let db = state.db()?;
    db.list_items(receipt_id)
}

#[tauri::command]
pub fn receipt_totals(
    state: State<'_, AppState>,
    receipt_id: &str,
) -> Result<ReceiptTotals, DbError> {
    let db = state.db()?;
    db.receipt_totals(receipt_id)
}

#[tauri::command]
pub fn finalize_receipt(state: State<'_, AppState>, receipt_id: &str) -> Result<usize, DbError> {
    let mut db = state.db()?;
    db.finalize_receipt(receipt_id)
}

#[tauri::command]
pub fn list_receipts(state: State<'_, AppState>) -> Result<Vec<Receipt>, DbError> {
    let db = state.db()?;
    db.list_receipts()
}

#[tauri::command]
pub fn list_products_with_stock(state: State<'_, AppState>) -> Result<Vec<ProductWithStock>, DbError> {
    state.db()?.list_products_with_stock()
}