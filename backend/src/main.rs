//! Sudoku backend — Axum web server.
//! Serves the Yew Wasm bundle as static files and exposes a JSON REST API.

use axum::{
    extract::Query,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use include_dir::{include_dir, Dir};
use mime_guess::from_path;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use sudoku_core::{validate, Board, Difficulty};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

// Embed the entire frontend dist/ directory at compile time.
// The Dockerfile ensures this directory exists before compilation.
static FRONTEND_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../frontend/dist");

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,backend=debug".to_string()),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // ── API routes ──
        .route("/api/puzzle", get(get_puzzle))
        .route("/api/validate", post(post_validate))
        .route("/api/solve", post(post_solve))
        .route("/api/health", get(health))
        // ── Static file catch-all (serves Wasm SPA) ──
        .fallback(static_handler)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ─── Health ───────────────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// ─── GET /api/puzzle ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PuzzleQuery {
    difficulty: Option<String>,
}

#[derive(Serialize)]
struct PuzzleResponse {
    board: Board,
    difficulty: Difficulty,
}

async fn get_puzzle(Query(q): Query<PuzzleQuery>) -> impl IntoResponse {
    let difficulty = q
        .difficulty
        .as_deref()
        .unwrap_or("medium")
        .parse::<Difficulty>()
        .unwrap_or(Difficulty::Medium);

    let board = sudoku_core::generate(difficulty);
    Json(PuzzleResponse { board, difficulty })
}

// ─── POST /api/validate ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ValidateRequest {
    board: Board,
}

#[derive(Serialize)]
struct ValidateResponse {
    result: sudoku_core::ValidationResult,
}

async fn post_validate(Json(body): Json<ValidateRequest>) -> impl IntoResponse {
    let result = validate(&body.board);
    Json(ValidateResponse { result })
}

// ─── POST /api/solve ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SolveRequest {
    board: Board,
}

#[derive(Serialize)]
struct SolveResponse {
    board: Option<Board>,
}

async fn post_solve(Json(body): Json<SolveRequest>) -> impl IntoResponse {
    let mut board = body.board.clone();
    let solved = sudoku_core::solve(&mut board);
    let result_board = if solved { Some(board) } else { None };
    Json(SolveResponse {
        board: result_board,
    })
}

// ─── Static file handler ─────────────────────────────────────────────────────

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try the exact path first, then fall back to index.html for SPA routing.
    let file = FRONTEND_DIR
        .get_file(path)
        .or_else(|| FRONTEND_DIR.get_file("index.html"));

    match file {
        Some(f) => {
            let mime = from_path(f.path()).first_or_octet_stream();
            let content_type = mime.as_ref().to_string();
            ([(header::CONTENT_TYPE, content_type)], f.contents()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}
