ALTER TABLE orders
ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

UPDATE orders
SET updated_at = COALESCE(updated_at, created_at, NOW())
WHERE updated_at IS NULL;

ALTER TABLE orders
ALTER COLUMN updated_at SET DEFAULT NOW(),
ALTER COLUMN updated_at SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'orders_status_allowed'
    ) THEN
        ALTER TABLE orders
        ADD CONSTRAINT orders_status_allowed
        CHECK (status IN ('pending', 'paid', 'shipped', 'completed', 'cancelled'));
    END IF;
END $$;
