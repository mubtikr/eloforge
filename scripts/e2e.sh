#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

API_PORT="${API_PORT:-8080}"
BASE="http://localhost:${API_PORT}"
USERNAME="${E2E_CHESSCOM_USERNAME:-erik}"
E2E_RESET="${E2E_RESET:-0}"

log() { printf '\n\033[1m%s\033[0m\n' "$*"; }
fail() { printf '\nE2E FAIL: %s\n' "$*" >&2; exit 1; }

psql_value() {
  docker compose exec -T postgres psql -U "${POSTGRES_USER:-eloforge}" -d "${POSTGRES_DB:-eloforge}" -tAc "$1" | head -1 | tr -d '[:space:]'
}

wait_health() {
  for _ in $(seq 1 90); do
    if curl -fsS "$BASE/api/healthz" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  fail "API health endpoint never became ready"
}

mint_token() {
  local user_id="$1" secret="$2"
  USER_ID="$user_id" JWT_SECRET_VALUE="$secret" python3 - <<'PY'
import base64, hashlib, hmac, json, os, time
def b64(data): return base64.urlsafe_b64encode(data).decode().rstrip('=')
uid = os.environ['USER_ID']; now = int(time.time())
header = b64(json.dumps({"alg": "HS256", "typ": "JWT"}, separators=(',', ':')).encode())
payload = b64(json.dumps({"sub": uid, "username": "e2e", "iat": now, "exp": now + 3600}, separators=(',', ':')).encode())
signature = b64(hmac.new(os.environ['JWT_SECRET_VALUE'].encode(), f"{header}.{payload}".encode(), hashlib.sha256).digest())
print(f"{header}.{payload}.{signature}")
PY
}

if [ "$E2E_RESET" = "1" ]; then
  log "Resetting the Docker stack and database (E2E_RESET=1)"
  docker compose down -v --remove-orphans || true
fi

log "Building and starting the stack"
SYNC_MAX_ARCHIVES="${SYNC_MAX_ARCHIVES:-1}" SYNC_INTERVAL_MINUTES=1440 docker compose up -d --build
wait_health
log "Health: $(curl -fsS "$BASE/api/healthz")"

log "Seeding an authenticated test player ($USERNAME)"
docker compose exec -T postgres psql -U "${POSTGRES_USER:-eloforge}" -d "${POSTGRES_DB:-eloforge}" -q -c \
  "INSERT INTO users (chesscom_username, display_name, current_elo, peak_elo) VALUES ('$USERNAME','E2E Player',1800,1900) ON CONFLICT (lower(chesscom_username)) DO UPDATE SET display_name = EXCLUDED.display_name;" >/dev/null
USER_ID="$(psql_value "SELECT id FROM users WHERE lower(chesscom_username)=lower('$USERNAME')")"
[ -n "$USER_ID" ] || fail "could not seed user"
JWT_SECRET_VALUE="$(docker compose exec -T api printenv JWT_SECRET | tr -d '[:space:]')"
TOKEN="$(mint_token "$USER_ID" "$JWT_SECRET_VALUE")"
AUTH=(-H "Cookie: ef_session=$TOKEN")
curl -fsS "${AUTH[@]}" "$BASE/api/auth/me" >/dev/null || fail "session cookie rejected"

log "Triggering Chess.com serial sync"
curl -fsS -X POST "${AUTH[@]}" "$BASE/api/sync/trigger" >/dev/null
for _ in $(seq 1 90); do
  STATUS="$(curl -fsS "${AUTH[@]}" "$BASE/api/sync/status")"
  RUNNING="$(printf '%s' "$STATUS" | python3 -c 'import json,sys; print(json.load(sys.stdin)["running"])')"
  TOTAL="$(printf '%s' "$STATUS" | python3 -c 'import json,sys; print(json.load(sys.stdin)["total_games"])')"
  if [ "$RUNNING" = "False" ] && [ "$TOTAL" -gt 0 ]; then
    log "Sync complete: $TOTAL game(s)"
    break
  fi
  sleep 2
done
TOTAL_GAMES="$(psql_value "SELECT COUNT(*) FROM games WHERE user_id='$USER_ID'")"
[ "${TOTAL_GAMES:-0}" -gt 0 ] || fail "sync imported no games"

log "Analysing games until the queue has a personal drill"
ANALYSED=0
ENDGAME_ID=""
for _ in $(seq 1 5); do
  GAME_ID="$(psql_value "SELECT id FROM games WHERE user_id='$USER_ID' AND analysis_status IN ('pending','failed') ORDER BY start_time DESC LIMIT 1")"
  if [ -z "$GAME_ID" ]; then
    GAME_ID="$(psql_value "SELECT id FROM games WHERE user_id='$USER_ID' ORDER BY start_time DESC LIMIT 1")"
    [ -n "$GAME_ID" ] || break
  fi
  curl -fsS -X POST "${AUTH[@]}" "$BASE/api/games/$GAME_ID/analyze" >/dev/null || true
  for _ in $(seq 1 90); do
    STATE="$(psql_value "SELECT analysis_status FROM games WHERE id='$GAME_ID'")"
    [ "$STATE" = "done" ] && break
    [ "$STATE" = "failed" ] && fail "analysis failed for $GAME_ID"
    sleep 2
  done
  [ "$(psql_value "SELECT analysis_status FROM games WHERE id='$GAME_ID'")" = "done" ] || fail "analysis timed out"
  ANALYSED=$((ANALYSED + 1))
  ENDGAME_ID="$(psql_value "SELECT id FROM puzzles WHERE user_id='$USER_ID' AND kind='endgame_conversion' ORDER BY created_at DESC LIMIT 1")"
  [ -n "$ENDGAME_ID" ] && break
done
[ "$ANALYSED" -gt 0 ] || fail "no game was analysed"

if [ -z "$ENDGAME_ID" ]; then
  log "No endgame conversion was tagged in this small sample; seeding a guaranteed K+Q mate drill"
  ENDGAME_ID="$(docker compose exec -T postgres psql -U "${POSTGRES_USER:-eloforge}" -d "${POSTGRES_DB:-eloforge}" -tAc \
    "INSERT INTO puzzles (user_id, kind, fen, solution_uci, solution_san, tags, epl, due_at)
     VALUES ('$USER_ID','endgame_conversion','7k/5Q2/6K1/8/8/8/8/8 w - - 0 1','f7g7','Qg7#',ARRAY['ENDGAME_TECHNIQUE_FAIL'],0.3,now())
     RETURNING id" | grep -E '^[0-9a-f-]{36}$' | head -1)"
fi

log "Submitting a reflection and streaming the coach response"
BAD_MOVE="$(docker compose exec -T postgres psql -U "${POSTGRES_USER:-eloforge}" -d "${POSTGRES_DB:-eloforge}" -tAc \
  "SELECT m.id || '|' || g.id FROM game_moves m JOIN games g ON g.id=m.game_id
   WHERE g.user_id='$USER_ID' AND m.color=g.user_color
     AND (m.classification IN ('inaccuracy','mistake','blunder','missed_win') OR cardinality(m.mistake_tags) > 0)
   ORDER BY COALESCE(m.epl,0) DESC LIMIT 1" | head -1)"
[ -n "$BAD_MOVE" ] || fail "no critical move available for reflection"
MOVE_ID="${BAD_MOVE%%|*}"
MOVE_GAME="${BAD_MOVE##*|}"
SSE_FILE="$(mktemp)"
curl -sN --max-time 60 -X POST "${AUTH[@]}" -H 'content-type: application/json' \
  -d '{"reason_tags":["didnt_see_defender"],"explanation":"I moved before checking the defender."}' \
  "$BASE/api/games/$MOVE_GAME/moves/$MOVE_ID/reflect" > "$SSE_FILE" || true
grep -q '^event: done' "$SSE_FILE" || fail "reflection stream did not finish"
grep -q 'next_time_rule' "$SSE_FILE" || fail "reflection stream did not return a next-time rule"
REFLECTION_ID="$(psql_value "SELECT id FROM move_reflections WHERE user_id='$USER_ID' ORDER BY created_at DESC LIMIT 1")"
[ -n "$REFLECTION_ID" ] || fail "reflection was not stored"
grep -E '^event: delta|^data: .*What happened' "$SSE_FILE" | head -4

log "Solving one due puzzle through the FSRS endpoint"
DUE="$(curl -fsS "${AUTH[@]}" "$BASE/api/puzzles/due?limit=1")"
PUZZLE_ID="$(printf '%s' "$DUE" | python3 -c 'import json,sys; data=json.load(sys.stdin); print(data[0]["id"] if data else "")')"
SOLUTION="$(printf '%s' "$DUE" | python3 -c 'import json,sys; data=json.load(sys.stdin); print(data[0]["solution_uci"] if data else "")')"
[ -n "$PUZZLE_ID" ] || fail "no due puzzle"
ATTEMPT="$(curl -fsS -X POST "${AUTH[@]}" -H 'content-type: application/json' \
  -d "{\"uci\":\"$SOLUTION\",\"hints\":0,\"elapsed_ms\":2400}" "$BASE/api/puzzles/$PUZZLE_ID/attempt")"
printf '%s' "$ATTEMPT" | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["correct"] is True; print("Puzzle scheduled:", data["interval_days"], "day(s)")'
EXPLANATION="$(curl -fsS "${AUTH[@]}" "$BASE/api/puzzles/$PUZZLE_ID/explanation")"
printf '%s' "$EXPLANATION" | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["goal"] and data["why"] and data["next_time_rule"]; print("Puzzle explanation:", data["goal"])'
FOCUS="$(curl -fsS "${AUTH[@]}" "$BASE/api/focus?tz=Africa%2FCairo")"
printf '%s' "$FOCUS" | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["timezone"] == "Africa/Cairo"; print("Focus timezone:", data["timezone"])'

log "Playing an endgame sparring move against the engine"
SESSION="$(curl -fsS -X POST "${AUTH[@]}" -H 'content-type: application/json' \
  -d "{\"puzzle_id\":\"$ENDGAME_ID\"}" "$BASE/api/sparring/start")"
SESSION_ID="$(printf '%s' "$SESSION" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
END_SOLUTION="$(psql_value "SELECT solution_uci FROM puzzles WHERE id='$ENDGAME_ID'")"
SPAR="$(curl -fsS -X POST "${AUTH[@]}" -H 'content-type: application/json' \
  -d "{\"uci\":\"$END_SOLUTION\"}" "$BASE/api/sparring/$SESSION_ID/move")"
printf '%s' "$SPAR" | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["status"] in ("won","active","draw"); print("Sparring:", data["status"], "|", data["message"])'

log "Checking Prometheus metrics"
METRICS="$(curl -fsS "$BASE/metrics")"
for series in eloforge_analysis_jobs_total eloforge_stockfish_queue_depth eloforge_ai_requests_total eloforge_sync_games_total; do
  printf '%s' "$METRICS" | grep -q "$series" || fail "missing metric $series"
done

log "E2E PASS — sync, analysis, puzzle extraction, reflection, FSRS and sparring all responded"
echo "games=$(psql_value "SELECT COUNT(*) FROM games WHERE user_id='$USER_ID'") analysed=$ANALYSED puzzles=$(psql_value "SELECT COUNT(*) FROM puzzles WHERE user_id='$USER_ID'") reflections=$(psql_value "SELECT COUNT(*) FROM move_reflections WHERE user_id='$USER_ID'")"
