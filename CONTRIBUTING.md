# Contributing

## Workflow

1. Branch from `main`: `feat/<name>`, `fix/<name>`, `docs/<name>`, `test/<name>`, `chore/<name>`
2. Conventional commits — `<type>: <subject>`, lowercase, imperative, no period, ≤72 chars; body explains *why*
3. All gates green before merge: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace -q` (Rust), `pnpm build` (frontend)
4. No AI attribution lines in commit messages (a `prepare-commit-msg` hook strips them — do not bypass it with `--no-verify`)

## Dependencies

- Cargo: add via `cargo add`; lockfile is committed; a new crate must be ≥7 days old on crates.io — force exact versions with `cargo update --precide <ver> -p <crate>` when the newest release is younger
- pnpm: exact pins; `pnpm-lock.yaml` committed; same ≥7-day rule for new packages

## Data

- `data/` is gitignored — never commit the raw Kaggle CSV or the built dataset
- Real datasets are fetched per user at setup (see README); tests use committed fixtures
- Classification/severity heuristics live in `crates/engine/src/dataset.rs` and are documented there — change them consciously, with tests
