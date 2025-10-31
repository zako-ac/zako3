-- Add up migration script here
CREATE TABLE taps (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);
