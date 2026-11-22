.DEFAULT_GOAL := help

.PHONY: help install dev down logs check backend-check frontend-check fmt test build clean backup

help:
	@printf "EloForge\n\n"
	@printf "  make install          Install frontend dependencies\n"
	@printf "  make dev              Build and start the full Docker stack\n"
	@printf "  make down             Stop the stack\n"
	@printf "  make logs             Follow API and web logs\n"
	@printf "  make check            Run every verification gate\n"
	@printf "  make backend-check    cargo fmt + clippy + test\n"
	@printf "  make frontend-check   typecheck + lint + build + impeccable\n"
	@printf "  make test             Rust tests\n"
	@printf "  make backup           Dump PostgreSQL to ./backups\n"

install:
	cd frontend && pnpm install --frozen-lockfile

dev:
	docker compose up --build

down:
	docker compose down

logs:
	docker compose logs -f api web

backend-check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

frontend-check:
	cd frontend && pnpm typecheck && pnpm lint && pnpm build && npx impeccable detect src/

check: backend-check frontend-check
	docker compose config >/dev/null

test:
	cargo test

build:
	cargo build --release
	cd frontend && pnpm build

clean:
	cargo clean
	rm -rf frontend/dist

backup:
	./scripts/backup.sh

e2e:
	./scripts/e2e.sh
