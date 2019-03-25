-- Your SQL goes here
CREATE TABLE groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The name of the group
    name TEXT NOT NULL,
    -- The ID of the owner of the group
    owner_id INTEGER NOT NULL,
    -- Optional room the group meets in
    room TEXT
);

INSERT INTO groups (id, name, owner_id) VALUES (0, "RCOS Large Group", 0);