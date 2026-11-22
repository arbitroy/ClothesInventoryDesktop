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
    #[error("Invalid date, expected YYYY-MM-DD")]
    InvalidDate,
    #[error("This day is already closed")]
    DayAlreadyClosed,
    #[error("Missing a count for {0}")]
    MissingCount(String),
    #[error("Counts can't be negative")]
    InvalidCount,
    #[error("You can't close a day that hasn't happened yet")]
    FutureDate,
    #[error("A later day is already closed, so this day can't be closed now")]
    DayOutOfOrder,
    #[error("That isn't a valid reason for stock going out")]
    InvalidReason,
    #[error("Invalid data: {0}")]
    BadData(#[from] serde_json::Error),
    #[error("This kind of change isn't supported")]
    UnsupportedChange,
    #[error("An update must say which version it started from")]
    MissingBaseVersion,
    #[error("This conflict doesn't exist or was already resolved")]
    ConflictNotPending,
    #[error("This product was deleted, so only the desktop version can be kept")]
    ProductDeleted,
    #[error("A stock-in must have the id in:<item id>")]
    BadStockInId,
    #[error("Not authorized")]
    Unauthorized,
    #[error("Wrong or expired pairing code")]
    PairingFailed,
}

pub type DbResult<T> = Result<T, DbError>;

// Tauri needs errors to be serializable. Send the message as plain text,
// so React's catch (e) still gets a string and nothing changes there.
impl serde::Serialize for DbError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
