pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS products (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            size       TEXT,
            notes      TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            deleted_at INTEGER
);

CREATE TABLE IF NOT EXISTS container_receipts (
    id              TEXT PRIMARY KEY,
    container_number TEXT NOT NULL,
    received_date   INTEGER NOT NULL,
    notes           TEXT,
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft', 'finalized')),
    version         INTEGER NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER
);

CREATE TABLE IF NOT EXISTS container_items (
    id                TEXT PRIMARY KEY,
    receipt_id        TEXT NOT NULL REFERENCES container_receipts(id),
    product_id        TEXT NOT NULL REFERENCES products(id),
    quantity_received INTEGER NOT NULL CHECK (quantity_received > 0),
    notes             TEXT,
    created_at        INTEGER NOT NULL
);
";