-- Enable foreign keys (good practice for SQLite)
PRAGMA foreign_keys = ON ;

-- Create houses (pre-populate)
CREATE TABLE houses (
id INTEGER PRIMARY KEY AUTOINCREMENT,
name TEXT UNIQUE NOT NULL,
score INTEGER DEFAULT 0
) ;

INSERT INTO houses (name) VALUES
('Gryffindor'), ('Hufflepuff'), ('Ravenclaw'), ('Slytherin') ;

-- Guests table
CREATE TABLE guests (
id INTEGER PRIMARY KEY AUTOINCREMENT,
name TEXT NOT NULL,
house_id INTEGER NOT NULL REFERENCES houses (id) ON DELETE RESTRICT,
personal_score INTEGER DEFAULT 0,
is_active INTEGER DEFAULT 1,
created_at DATETIME DEFAULT CURRENT_TIMESTAMP
) ;

CREATE INDEX idx_guests_house_active ON guests (house_id) WHERE is_active = 1 ;

-- Sessions table (one per guest; cascade delete on guest unregister)
CREATE TABLE sessions (
id INTEGER PRIMARY KEY AUTOINCREMENT,
guest_id INTEGER NOT NULL UNIQUE REFERENCES guests (id) ON DELETE CASCADE,
token TEXT UNIQUE NOT NULL,
created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
expires_at DATETIME
) ;

-- Point awards (audit log; one of guest_id or house_id must be set)
CREATE TABLE point_awards (
id INTEGER PRIMARY KEY AUTOINCREMENT,
guest_id INTEGER REFERENCES guests (id) ON DELETE SET NULL,
house_id INTEGER REFERENCES houses (id) ON DELETE SET NULL,
amount INTEGER NOT NULL,
reason TEXT NOT NULL,
awarded_at DATETIME DEFAULT CURRENT_TIMESTAMP,
CHECK ((guest_id IS NOT NULL AND house_id IS NULL) OR (guest_id IS NULL AND house_id IS NOT NULL))
) ;
