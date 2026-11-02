-- Add migration script here
ALTER TABLE deposits ALTER COLUMN amount TYPE BIGINT;
