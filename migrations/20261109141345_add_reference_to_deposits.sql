-- Add migration script here
ALTER TABLE deposits ADD COLUMN reference TEXT UNIQUE NOT NULL;
