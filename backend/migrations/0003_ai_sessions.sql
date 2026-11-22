-- Stable OpenCode Go session per game review, so prompt caching and routing
-- affinity work across reflection turns.
ALTER TABLE games
    ADD COLUMN IF NOT EXISTS ai_session_id UUID NOT NULL DEFAULT gen_random_uuid();
