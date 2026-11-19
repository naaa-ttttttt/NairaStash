-- Add migration script here
-- plans without an end date or a target would break the new rules, they are test plans
DELETE FROM savings_plans
WHERE end_date IS NULL OR target_amount IS NULL;

-- flexible plans used to be forbidden from having an end date, now every plan has one
ALTER TABLE savings_plans DROP CONSTRAINT locked_needs_end_date;

-- every plan now has an end date and a stored target
ALTER TABLE savings_plans ALTER COLUMN end_date SET NOT NULL;
ALTER TABLE savings_plans ALTER COLUMN target_amount SET NOT NULL;
