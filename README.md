# Sudoku Rust App

A full-stack Sudoku web application built entirely in Rust, deployed on Google Cloud Run.

## Stack

| Layer | Technology |
|-------|-----------|
| Game logic | `sudoku-core` — pure Rust library (generator, solver, validator) |
| Frontend | [Yew](https://yew.rs) — Rust-compiled WebAssembly SPA |
| Backend | [Axum](https://github.com/tokio-rs/axum) — async Rust web server |
| Container | Docker multi-stage build |
| CI/CD | GitHub Actions |
| Hosting | Google Cloud Run (us-central1) |

## Project Structure

```
.
├── sudoku-core/          # Shared library: board, generator, solver, validator
├── frontend/             # Yew SPA (compiled to Wasm via Trunk)
├── backend/              # Axum REST API + static file server
├── Dockerfile            # Multi-stage: Wasm build → native build → slim runtime
├── .github/workflows/
│   └── ci-cd.yml         # Test + lint on every push/PR; deploy to Cloud Run on main
└── scripts/
    └── gcp-setup.sh      # One-time GCP infrastructure setup
```

## REST API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/puzzle?difficulty=easy\|medium\|hard` | Generate a new puzzle |
| `POST` | `/api/validate` | Validate current board state |
| `POST` | `/api/solve` | Return the full solution |
| `GET` | `/api/health` | Health check |

## CI/CD Pipeline

Every **push or pull request to `main`** triggers:

1. **`test` job** — `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`
2. **`build-and-deploy` job** (push to `main` only, after tests pass):
   - Builds the multi-stage Docker image
   - Pushes to Artifact Registry (`us-central1-docker.pkg.dev/opencode-sudoku-rust-app/sudoku-app/app`)
   - Deploys to Cloud Run (`sudoku-app` service, `us-central1`)

Authentication uses **Workload Identity Federation** (no long-lived service account keys stored as secrets).

## First-Time GCP Setup

Run once from a machine with `gcloud` configured:

```bash
gcloud auth login
gcloud config set project opencode-sudoku-rust-app
./scripts/gcp-setup.sh
```

Then add the three printed values as **GitHub Secrets** in the repo settings:
- `GCP_PROJECT_ID`
- `GCP_SERVICE_ACCOUNT`
- `GCP_WORKLOAD_IDENTITY_PROVIDER`

## Local Development

### Prerequisites

- Rust stable (`rustup update stable`)
- `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
- [Trunk](https://trunkrs.dev/) (`cargo install trunk`)

### Run backend + frontend

```bash
# Terminal 1 — build and watch frontend
cd frontend
trunk serve --proxy-backend=http://localhost:8080/api

# Terminal 2 — run backend
RUST_LOG=debug cargo run -p backend
```

Open http://localhost:8080 in your browser.

### Run tests

```bash
cargo test --workspace --exclude frontend
```

## License

MIT
