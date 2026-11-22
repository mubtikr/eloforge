#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

DUMP="${1:-}"
POSTGRES_USER="${POSTGRES_USER:-eloforge}"
POSTGRES_DB="${POSTGRES_DB:-eloforge}"

if [ -z "$DUMP" ] || [ ! -f "$DUMP" ]; then
  echo "Usage: ./scripts/restore.sh backups/eloforge-YYYYmmdd-HHMMSS.dump" >&2
  exit 1
fi

read -r -p "This will replace the current $POSTGRES_DB database. Type 'restore' to continue: " answer
if [ "$answer" != "restore" ]; then
  echo "Aborted."
  exit 1
fi

docker compose stop api
docker compose exec -T postgres \
  pg_restore -U "$POSTGRES_USER" -d "$POSTGRES_DB" --clean --if-exists --no-owner --no-privileges \
  < "$DUMP"
docker compose start api
echo "Restore complete."
