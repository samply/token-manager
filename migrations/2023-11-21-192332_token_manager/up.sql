-- Your SQL goes here

CREATE TABLE tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token_name TEXT NOT NULL,
    token TEXT NOT NULL, 
    project_id TEXT NOT NULL, 
    bk TEXT NOT NULL, 
    token_status TEXT NOT NULL,
    project_status TEXT NOT NULL,  
    user_id TEXT NOT NULL,
    token_created_at TEXT NOT NULL
    )