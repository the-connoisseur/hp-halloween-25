CREATE TABLE voting_status (
id INTEGER PRIMARY KEY,
is_open INTEGER NOT NULL DEFAULT 0 CHECK (is_open IN (0, 1)),
opened_at TIMESTAMP,
closed_at TIMESTAMP
);

INSERT INTO voting_status (id, is_open) VALUES (1, 0);

CREATE TABLE votes (
id INTEGER PRIMARY KEY AUTOINCREMENT,
voter_id INTEGER NOT NULL,
first_choice_id INTEGER NOT NULL,
second_choice_id INTEGER NOT NULL,
third_choice_id INTEGER NOT NULL,
submitted_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
UNIQUE(voter_id),
FOREIGN KEY (voter_id) REFERENCES guests(id) ON DELETE CASCADE,
FOREIGN KEY (first_choice_id) REFERENCES guests(id) ON DELETE RESTRICT,
FOREIGN KEY (second_choice_id) REFERENCES guests(id) ON DELETE RESTRICT,
FOREIGN KEY (third_choice_id) REFERENCES guests(id) ON DELETE RESTRICT,
CHECK (first_choice_id != voter_id AND second_choice_id != voter_id AND third_choice_id != voter_id),
CHECK (first_choice_id != second_choice_id AND second_choice_id != third_choice_id AND third_choice_id != first_choice_id)
);

CREATE INDEX idx_votes_voter ON votes(voter_id);
CREATE INDEX idx_votes_submitted_at ON votes(submitted_at);

