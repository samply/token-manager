-- Your SQL goes here

CREATE TABLE tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token TEXT NOT NULL, 
    project_id TEXT NOT NULL, 
    bk TEXT NOT NULL, 
    status TEXT NOT NULL, 
    user_id TEXT NOT NULL
    )