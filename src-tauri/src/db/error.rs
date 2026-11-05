use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Receipt not found")]
    ReceiptNotFound,
    #[error("This receipt is already finalized")]
    ReceiptFinalized,
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Add at least one item before finalizing")]
    EmptyReceipt,
    #[error("This product still has stock ({0}). Bring it to zero before deleting")]
    ProductHasStock(i64),
    #[error("Product not found")]
    ProductNotFound,
    #[error("Quantity must be at least 1")]
    InvalidQuantity,
    #[error("This flag doesn't exist or was already resolved")]
    FlagNotPending,
}

pub type DbResult<T> = Result<T, DbError>;

// Tauri needs errors to be serializable. Send the message as plain text,
// so React's catch (e) still gets a string and nothing changes there.
impl serde::Serialize for DbError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
