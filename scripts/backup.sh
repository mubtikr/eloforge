#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BACKUP_DIR="${BACKUP_DIR:-backups}"
RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-14}"
POSTGRES_USER="${POSTGRES_USER:-eloforge}"
POSTGRES_DB="${POSTGRES_DB:-eloforge}"

mkdir -p "$BACKUP_DIR"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
TARGET="$BACKUP_DIR/eloforge-$STAMP.dump"

echo "Dumping $POSTGRES_DB to $TARGET"
docker compose exec -T postgres \
  pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --format=custom --no-owner --no-privileges \
  > "$TARGET"

if [ ! -s "$TARGET" ]; then
  echo "Backup is empty; removing $TARGET" >&2
  rm -f "$TARGET"
  exit 1
fi

echo "Backup written: $(du -h "$TARGET" | cut -f1)"
find "$BACKUP_DIR" -name 'eloforge-*.dump' -type f -mtime +"$RETENTION_DAYS" -print -delete

cat <<MSG

Restore with:
  ./scripts/restore.sh $TARGET

Schedule with cron (daily 03:15):
  15 3 * * * cd $(pwd) && ./scripts/backup.sh >> logs/backup.log 2>&1
MSG
