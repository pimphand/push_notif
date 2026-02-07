-- Users untuk login
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Tabel key: menyimpan key, public_key, domain
CREATE TABLE IF NOT EXISTS keys (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    key TEXT NOT NULL,
    public_key TEXT NOT NULL,
    domain VARCHAR(512) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- User default akan dibuat oleh aplikasi (seed) jika belum ada
