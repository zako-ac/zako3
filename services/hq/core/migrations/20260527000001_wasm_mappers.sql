CREATE TABLE wasm_mappers (
    id           TEXT        PRIMARY KEY,
    name         TEXT        NOT NULL,
    wasm_bytes   BYTEA       NOT NULL,
    sha256_hash  TEXT        NOT NULL,
    input_data   JSONB       NOT NULL DEFAULT '[]'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE pipeline_order (
    position  INTEGER PRIMARY KEY,
    mapper_id TEXT    NOT NULL REFERENCES wasm_mappers(id) ON DELETE CASCADE
);
