-- Only one active sparring session per player. Older active rows are kept for
-- history but marked abandoned so the Arena never shows two "active" drills.
WITH ranked AS (
    SELECT id,
           row_number() OVER (PARTITION BY user_id ORDER BY created_at DESC) AS rn
    FROM sparring_sessions
    WHERE status = 'active'
)
UPDATE sparring_sessions s
SET status = 'abandoned', result = 'abandoned'
FROM ranked r
WHERE s.id = r.id AND r.rn > 1;
