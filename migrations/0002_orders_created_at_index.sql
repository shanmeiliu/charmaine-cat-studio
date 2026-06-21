-- migrations/0003_orders_created_at_index.sql

CREATE INDEX IF NOT EXISTS idx_orders_created_at
ON orders(created_at DESC);