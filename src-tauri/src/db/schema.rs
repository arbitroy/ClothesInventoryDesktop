pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS products (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            size       TEXT,
            notes      TEXT,
            version    INTEGER NOT NULL DEFAULT 1,
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
    reason TEXT CHECK (reason IN ('sales', 'damage', 'adjustment')),
    quantity        INTEGER NOT NULL CHECK (quantity > 0),
    user_id         TEXT,
    notes           TEXT,
    receipt_item_id TEXT REFERENCES container_items(id),
    price_per_unit  INTEGER,   -- minor units (e.g. cents), not decimals
    total_price     INTEGER,
    timestamp       INTEGER NOT NULL,
    CHECK (
    (type = 'in'  AND (reason IS NULL OR reason = 'adjustment')) OR
    (type = 'out' AND reason IS NOT NULL)
    )
);

CREATE TABLE IF NOT EXISTS review_flags (
    id          TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,   -- 'transaction' for now
    entity_id   TEXT NOT NULL,
    reason      TEXT NOT NULL,   -- 'negative_stock'
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'approved', 'rejected')),
    created_at  INTEGER NOT NULL,
    resolved_at INTEGER
);

CREATE TABLE IF NOT EXISTS daily_balances (
    date             TEXT NOT NULL,   -- 'YYYY-MM-DD'
    product_id       TEXT NOT NULL REFERENCES products(id),
    opening_balance  INTEGER NOT NULL,
    total_in         INTEGER NOT NULL,
    total_out        INTEGER NOT NULL,
    expected_balance INTEGER NOT NULL,
    closing_balance  INTEGER NOT NULL CHECK (closing_balance >= 0),
    variance         INTEGER NOT NULL,
    variance_notes   TEXT,
    closed_by        TEXT NOT NULL,
    closed_at        INTEGER NOT NULL,
    PRIMARY KEY (date, product_id)
);
";