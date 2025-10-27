CREATE TABLE house_crossword_completions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  house_id INTEGER NOT NULL REFERENCES houses(id) ON DELETE CASCADE,
  word_index INTEGER NOT NULL CHECK (word_index >= 0 AND word_index <=6),
  completed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(house_id, word_index)
);

