-- EloForge initial schema
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS users (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chesscom_username   TEXT NOT NULL UNIQUE,
    chesscom_player_id  TEXT,
    display_name        TEXT,
    country             TEXT,
    avatar_url          TEXT,
    title               TEXT,
    current_elo         INTEGER,
    peak_elo            INTEGER,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS auth_challenges (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username        TEXT NOT NULL,
    nonce           TEXT NOT NULL,
    user_id         UUID REFERENCES users(id) ON DELETE CASCADE,
    expires_at      TIMESTAMPTZ NOT NULL,
    verified_at     TIMESTAMPTZ,
    ip_hash         TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_auth_challenges_nonce ON auth_challenges (nonce);
CREATE INDEX IF NOT EXISTS idx_auth_challenges_expires ON auth_challenges (expires_at);

CREATE TABLE IF NOT EXISTS games (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    chesscom_game_id    TEXT NOT NULL,
    url                 TEXT,
    pgn                 TEXT NOT NULL,
    initial_fen         TEXT NOT NULL DEFAULT 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
    rules               TEXT NOT NULL DEFAULT 'chess',
    time_class          TEXT,
    time_control        TEXT,
    rated               BOOLEAN NOT NULL DEFAULT true,
    white_username      TEXT,
    black_username      TEXT,
    white_rating        INTEGER,
    black_rating        INTEGER,
    user_color          TEXT NOT NULL CHECK (user_color IN ('white', 'black')),
    result              TEXT NOT NULL CHECK (result IN ('win', 'loss', 'draw')),
    result_reason       TEXT,
    start_time          TIMESTAMPTZ NOT NULL,
    end_time            TIMESTAMPTZ,
    move_count          INTEGER NOT NULL DEFAULT 0,
    opening_eco         TEXT,
    opening_name        TEXT,
    opening_variation   TEXT,
    last_book_ply       INTEGER,
    analysis_status     TEXT NOT NULL DEFAULT 'pending'
        CHECK (analysis_status IN ('pending', 'queued', 'analyzing', 'done', 'failed')),
    analysis_error      TEXT,
    accuracy_user       DOUBLE PRECISION,
    accuracy_white      DOUBLE PRECISION,
    accuracy_black      DOUBLE PRECISION,
    eval_peak           DOUBLE PRECISION,
    max_advantage       DOUBLE PRECISION,
    session_id          UUID,
    analysis            JSONB,
    analyzed_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, chesscom_game_id)
);
CREATE INDEX IF NOT EXISTS idx_games_user_start ON games (user_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_games_status ON games (user_id, analysis_status);
CREATE INDEX IF NOT EXISTS idx_games_opening ON games (user_id, opening_eco);
CREATE INDEX IF NOT EXISTS idx_games_session ON games (user_id, session_id);

CREATE TABLE IF NOT EXISTS game_moves (
    id                  BIGSERIAL PRIMARY KEY,
    game_id             UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    ply                 INTEGER NOT NULL,
    move_number         INTEGER NOT NULL,
    color               TEXT NOT NULL CHECK (color IN ('white', 'black')),
    san                 TEXT NOT NULL,
    uci                 TEXT NOT NULL,
    fen_before          TEXT NOT NULL,
    fen_after           TEXT NOT NULL,
    clock_remaining_sec DOUBLE PRECISION,
    time_spent_sec      DOUBLE PRECISION,
    eval_before_cp      INTEGER,
    eval_after_cp       INTEGER,
    eval_cp             INTEGER,
    eval_mate           INTEGER,
    win_prob_user       DOUBLE PRECISION,
    best_move_uci       TEXT,
    best_move_san       TEXT,
    epl                 DOUBLE PRECISION,
    classification      TEXT,
    mistake_tags        TEXT[] NOT NULL DEFAULT '{}',
    pv                  JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (game_id, ply)
);
CREATE INDEX IF NOT EXISTS idx_game_moves_game ON game_moves (game_id, ply);
CREATE INDEX IF NOT EXISTS idx_game_moves_classification ON game_moves (game_id, classification);

CREATE TABLE IF NOT EXISTS move_reflections (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id         UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    move_id         BIGINT NOT NULL REFERENCES game_moves(id) ON DELETE CASCADE,
    reason_tags     TEXT[] NOT NULL DEFAULT '{}',
    explanation     TEXT NOT NULL,
    ai_feedback     TEXT,
    next_time_rule  TEXT,
    ai_status       TEXT,
    embedding       vector(384),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reflections_user ON move_reflections (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_reflections_game ON move_reflections (game_id);
CREATE INDEX IF NOT EXISTS idx_reflections_tags ON move_reflections USING GIN (reason_tags);
CREATE INDEX IF NOT EXISTS idx_reflections_embedding ON move_reflections
    USING hnsw (embedding vector_cosine_ops);

CREATE TABLE IF NOT EXISTS puzzles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id         UUID REFERENCES games(id) ON DELETE CASCADE,
    move_id         BIGINT REFERENCES game_moves(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN ('blunder_fix', 'endgame_conversion')),
    fen             TEXT NOT NULL,
    solution_uci    TEXT NOT NULL,
    solution_san    TEXT,
    best_line       JSONB,
    tags            TEXT[] NOT NULL DEFAULT '{}',
    epl             DOUBLE PRECISION,
    due_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    stability       DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    difficulty      DOUBLE PRECISION NOT NULL DEFAULT 5.0,
    reps            INTEGER NOT NULL DEFAULT 0,
    lapses          INTEGER NOT NULL DEFAULT 0,
    state           TEXT NOT NULL DEFAULT 'new' CHECK (state IN ('new', 'learning', 'review')),
    mastered        BOOLEAN NOT NULL DEFAULT false,
    last_rating     INTEGER,
    last_reviewed_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, game_id, move_id, kind)
);
CREATE INDEX IF NOT EXISTS idx_puzzles_due ON puzzles (user_id, mastered, due_at)
    WHERE mastered = false;
CREATE INDEX IF NOT EXISTS idx_puzzles_kind ON puzzles (user_id, kind);

CREATE TABLE IF NOT EXISTS puzzle_attempts (
    id          BIGSERIAL PRIMARY KEY,
    puzzle_id   UUID NOT NULL REFERENCES puzzles(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    correct     BOOLEAN NOT NULL,
    hints       INTEGER NOT NULL DEFAULT 0,
    elapsed_ms  INTEGER,
    rating      INTEGER,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_puzzle_attempts_user ON puzzle_attempts (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS coaching_reports (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id     UUID REFERENCES games(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,
    title       TEXT NOT NULL,
    content     JSONB NOT NULL,
    model       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reports_user_kind ON coaching_reports (user_id, kind, created_at DESC);

CREATE TABLE IF NOT EXISTS milestones_and_goals (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('milestone', 'goal', 'reminder')),
    stage       INTEGER,
    title       TEXT NOT NULL,
    description TEXT,
    target_elo  INTEGER,
    target_date DATE,
    progress    JSONB NOT NULL DEFAULT '{}'::jsonb,
    completed   BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_goals_user ON milestones_and_goals (user_id, kind, stage);

CREATE TABLE IF NOT EXISTS sync_cursors (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url             TEXT NOT NULL,
    etag            TEXT,
    last_modified   TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    games_imported  INTEGER NOT NULL DEFAULT 0,
    last_synced_at  TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, url)
);

CREATE TABLE IF NOT EXISTS notification_settings (
    user_id                 UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    telegram_chat_id        TEXT,
    email                   TEXT,
    web_push_subscription   JSONB,
    daily_reminder          BOOLEAN NOT NULL DEFAULT true,
    tilt_alerts             BOOLEAN NOT NULL DEFAULT true,
    reminder_hour           INTEGER NOT NULL DEFAULT 9 CHECK (reminder_hour BETWEEN 0 AND 23),
    timezone                TEXT NOT NULL DEFAULT 'UTC',
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS notifications_log (
    id          BIGSERIAL PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel     TEXT NOT NULL,
    kind        TEXT NOT NULL,
    payload     JSONB NOT NULL DEFAULT '{}'::jsonb,
    sent_at     TIMESTAMPTZ,
    error       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ai_token_usage (
    id          BIGSERIAL PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day         DATE NOT NULL,
    tokens      BIGINT NOT NULL DEFAULT 0,
    requests    INTEGER NOT NULL DEFAULT 0,
    UNIQUE (user_id, day)
);

CREATE TABLE IF NOT EXISTS sparring_sessions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id     UUID REFERENCES games(id) ON DELETE SET NULL,
    puzzle_id   UUID REFERENCES puzzles(id) ON DELETE SET NULL,
    fen         TEXT NOT NULL,
    initial_fen TEXT NOT NULL,
    moves       JSONB NOT NULL DEFAULT '[]'::jsonb,
    status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'won', 'lost', 'draw', 'abandoned')),
    result      TEXT,
    user_elo    INTEGER,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_sparring_user ON sparring_sessions (user_id, status, created_at DESC);

CREATE OR REPLACE FUNCTION ef_set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    tbl TEXT;
BEGIN
    FOREACH tbl IN ARRAY ARRAY['users', 'games', 'move_reflections', 'puzzles',
                                'milestones_and_goals', 'notification_settings', 'sparring_sessions']
    LOOP
        EXECUTE format('DROP TRIGGER IF EXISTS trg_%s_updated_at ON %I', tbl, tbl);
        EXECUTE format(
            'CREATE TRIGGER trg_%s_updated_at BEFORE UPDATE ON %I
             FOR EACH ROW EXECUTE FUNCTION ef_set_updated_at()', tbl, tbl);
    END LOOP;
END;
$$;
