CREATE TABLE crossword_states (
id INTEGER PRIMARY KEY AUTOINCREMENT,
guest_id INTEGER NOT NULL REFERENCES guests(id) ON DELETE CASCADE,
state TEXT NOT NULL,  -- e.g., {"grid": [["", "", ...], ...], "completions": [false, false, ...]}
updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
) ;

-- Index for quick lookup by guest.
CREATE INDEX idx_crossword_guest ON crossword_states(guest_id) ;
