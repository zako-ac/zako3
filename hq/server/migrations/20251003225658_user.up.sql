-- Add up migration script here
CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    permissions BIGINT NOT NULL
);
