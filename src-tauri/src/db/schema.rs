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

CREATE TABLE IF NOT EXISTS transactions (
    id              TEXT PRIMARY KEY,
    product_id      TEXT NOT NULL REFERENCES products(id),
    type            TEXT NOT NULL CHECK (type IN ('in', 'out')),
    out_reason      TEXT CHECK (out_reason IN ('sales', 'damage', 'adjustment')),
    quantity        INTEGER NOT NULL CHECK (quantity > 0),
    user_id         TEXT,
    notes           TEXT,
    receipt_item_id TEXT REFERENCES container_items(id),
    price_per_unit  INTEGER,   -- minor units (e.g. cents), not decimals
    total_price     INTEGER,
    timestamp       INTEGER NOT NULL,
    CHECK (
        (type = 'in'  AND out_reason IS NULL) OR
        (type = 'out' AND out_reason IS NOT NULL)
    )
);g
";