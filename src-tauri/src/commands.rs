use crate::AppState;
use crate::db::{
    ConflictView, ContainerItem, DayCount, DayPreviewRow, DbError, Device, PendingFlag, Product,
    ProductWithStock, Receipt, ReceiptTotals,
};
use crate::server::Pairing;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingInfo {
    pub code: String,
    pub address: Option<String>,
}

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
pub fn list_products_with_stock(
    state: State<'_, AppState>,
) -> Result<Vec<ProductWithStock>, DbError> {
    state.db()?.list_products_with_stock()
}

#[tauri::command]
pub fn list_pending_flags(state: State<'_, AppState>) -> Result<Vec<PendingFlag>, DbError> {
    state.db()?.list_pending_flags()
}

#[tauri::command]
pub fn resolve_flag(
    state: State<'_, AppState>,
    flag_id: &str,
    approve: bool,
) -> Result<(), DbError> {
    let db = state.db()?;
    db.resolve_flag(flag_id, approve)
}

#[tauri::command]
pub fn day_preview(state: State<'_, AppState>, date: &str) -> Result<Vec<DayPreviewRow>, DbError> {
    let db = state.db()?;
    db.day_preview(date)
}

#[tauri::command]
pub fn close_day(
    state: State<'_, AppState>,
    date: &str,
    counts: Vec<DayCount>,
    closed_by: &str,
) -> Result<usize, DbError> {
    let mut db = state.db()?;
    db.close_day(date, &counts, closed_by)
}

#[tauri::command]
pub fn start_pairing(pairing: State<'_, Pairing>) -> Result<PairingInfo, DbError> {
    let code = pairing.start()?;
    let address = local_ip_address::local_ip()
        .ok()
        .map(|ip| format!("{ip}:8787"));
    Ok(PairingInfo { code, address })
}

#[tauri::command]
pub fn list_devices(state: State<'_, AppState>) -> Result<Vec<Device>, DbError> {
    let db = state.db()?;
    db.list_devices()
}

#[tauri::command]
pub fn revoke_device(state: State<'_, AppState>, id: &str) -> Result<usize, DbError> {
    let db = state.db()?;
    db.revoke_device(id)
}

#[tauri::command]
pub fn list_pending_conflicts(state: State<'_, AppState>) -> Result<Vec<ConflictView>, DbError> {
    let db = state.db()?;
    db.list_pending_conflicts()
}

#[tauri::command]
pub fn resolve_conflict(
    state: State<'_, AppState>,
    conflict_id: &str,
    use_phone: bool,
) -> Result<(), DbError> {
    let mut db = state.db()?;
    db.resolve_conflict(conflict_id, use_phone)
}
