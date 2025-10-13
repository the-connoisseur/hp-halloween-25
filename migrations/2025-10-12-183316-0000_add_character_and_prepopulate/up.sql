-- Drop the existing guests table (losing all data) and recreate with updated
-- schema:
-- - house_id now nullable for unregistered guests
-- - Added character TEXT column (nullable)
-- - Renamed created_at to registered_at (nullable, set on registration)
-- Drop the existing table.
DROP TABLE IF EXISTS guests;

-- Create new guests table with updated schema.
CREATE TABLE guests (
id INTEGER PRIMARY KEY AUTOINCREMENT,
name TEXT NOT NULL,
house_id INTEGER REFERENCES houses (id),  -- Nullable
personal_score INTEGER NOT NULL DEFAULT 0,
is_active INTEGER NOT NULL DEFAULT 0,
registered_at TIMESTAMP,
character TEXT  -- New nullable field
) ;

-- Pre-populate with unregistered guests (is_active=0, house_id=NULL,
-- character=NULL).
INSERT INTO guests (name,
personal_score,
is_active,
registered_at,
house_id,
character)
VALUES
("Leila S", 0, 0, NULL, NULL, NULL),
("Gautam A", 0, 0, NULL, NULL, NULL),
("Parvathi C", 0, 0, NULL, NULL, NULL),
("Rohit M", 0, 0, NULL, NULL, NULL),
("Annie M", 0, 0, NULL, NULL, NULL),
("Hari S", 0, 0, NULL, NULL, NULL),
("Dimple K", 0, 0, NULL, NULL, NULL),
("Pavan H", 0, 0, NULL, NULL, NULL),
("Vivek R", 0, 0, NULL, NULL, NULL),
("Karthik S", 0, 0, NULL, NULL, NULL),
("Mithun B", 0, 0, NULL, NULL, NULL),
("Sriram R", 0, 0, NULL, NULL, NULL),
("Harsha B", 0, 0, NULL, NULL, NULL),
("Pulkit M", 0, 0, NULL, NULL, NULL),
("Serena C", 0, 0, NULL, NULL, NULL),
("Ray K", 0, 0, NULL, NULL, NULL),
("Alekhya A", 0, 0, NULL, NULL, NULL),
("Kaushik B", 0, 0, NULL, NULL, NULL),
("Koushik B", 0, 0, NULL, NULL, NULL),
("Dhrithi C", 0, 0, NULL, NULL, NULL),
("Sourav S", 0, 0, NULL, NULL, NULL),
("Poorva R", 0, 0, NULL, NULL, NULL),
("Anant M", 0, 0, NULL, NULL, NULL),
("Purva T", 0, 0, NULL, NULL, NULL),
("Vaibhav J", 0, 0, NULL, NULL, NULL),
("Aparna V", 0, 0, NULL, NULL, NULL),
("Rohan T", 0, 0, NULL, NULL, NULL),
("Siddhi R", 0, 0, NULL, NULL, NULL),
("Manu K", 0, 0, NULL, NULL, NULL),
("Ansu G", 0, 0, NULL, NULL, NULL),
("Vipin M", 0, 0, NULL, NULL, NULL),
("Elisha G", 0, 0, NULL, NULL, NULL),
("Daniel I", 0, 0, NULL, NULL, NULL),
("Jade P", 0, 0, NULL, NULL, NULL),
("Sameer K", 0, 0, NULL, NULL, NULL),
("Akanksha B", 0, 0, NULL, NULL, NULL),
("Abhinav B", 0, 0, NULL, NULL, NULL) ;
