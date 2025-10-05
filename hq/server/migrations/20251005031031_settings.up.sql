-- Add up migration script here
CREATE TABLE settings (
    scope TEXT PRIMARY KEY,
    data JSONB NOT NULL
);
