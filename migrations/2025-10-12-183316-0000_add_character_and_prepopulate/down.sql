-- Reverse: Drop the new guests table (losing prepopulated data) and recreate
-- the original schema.

-- Drop the current table.
DROP TABLE IF EXISTS guests;

-- Recreate original guests table.
CREATE TABLE guests (
id INTEGER PRIMARY KEY AUTOINCREMENT,
name TEXT NOT NULL,
house_id INTEGER NOT NULL,
personal_score INTEGER NOT NULL DEFAULT 0,
is_active INTEGER NOT NULL DEFAULT 0,
created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
) ;
