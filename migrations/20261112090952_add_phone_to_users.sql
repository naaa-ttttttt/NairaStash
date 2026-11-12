-- Add migration script here
-- add the column as nullable first so existing rows do not break it
ALTER TABLE users ADD COLUMN phone text;

-- give every existing row a unique placeholder, the seeded user included
UPDATE users SET phone = 'placeholder-' || user_id::text WHERE phone IS NULL;

-- now every row has a value, so it can be required
ALTER TABLE users ALTER COLUMN phone SET NOT NULL;

-- no two users can share a phone number
ALTER TABLE users ADD CONSTRAINT users_phone_key UNIQUE (phone);
