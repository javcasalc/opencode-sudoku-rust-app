# ─────────────────────────────────────────────────────────────────────────────
# Stage 1: Build the Yew/Wasm frontend with Trunk
# ─────────────────────────────────────────────────────────────────────────────
FROM rust:1.77-slim AS frontend-builder

# Install system dependencies needed for Wasm toolchain
RUN apt-get update && apt-get install -y \
    curl \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install wasm-pack and trunk
RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk --locked --version "0.20.3"
RUN cargo install wasm-bindgen-cli --locked --version "0.2.92"

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY sudoku-core/ ./sudoku-core/
COPY frontend/ ./frontend/

# Build the frontend Wasm bundle
WORKDIR /app/frontend
RUN trunk build --release

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2: Build the Axum backend (native binary)
# ─────────────────────────────────────────────────────────────────────────────
FROM rust:1.77-slim AS backend-builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY sudoku-core/ ./sudoku-core/
COPY backend/ ./backend/

# Copy the built frontend dist/ so include_dir! can embed it at compile time
COPY --from=frontend-builder /app/frontend/dist ./frontend/dist

# Build only the backend crate in release mode
RUN cargo build --release -p backend

# ─────────────────────────────────────────────────────────────────────────────
# Stage 3: Minimal runtime image
# ─────────────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled backend binary
COPY --from=backend-builder /app/target/release/backend ./backend

# Cloud Run sets PORT env var; default to 8080
ENV PORT=8080
EXPOSE 8080

CMD ["./backend"]
