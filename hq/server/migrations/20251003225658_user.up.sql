-- Add up migration script here
CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    name TEXT NULL,
    permissions BIGINT NOT NULL
);
