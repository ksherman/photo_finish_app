-- PhotoFinish Event Bundle Schema v2
-- Adds untaxed print shipping & handling (`shipping_cents`) to orders.

ALTER TABLE orders ADD COLUMN shipping_cents INTEGER NOT NULL DEFAULT 0;
