//! Sudoku frontend — Yew WebAssembly SPA.

use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use serde::{Deserialize, Serialize};
use sudoku_core::{Board, Difficulty, ValidationResult};
use yew::prelude::*;

// ─── API helpers ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ValidateRequest<'a> {
    board: &'a Board,
}

#[derive(Serialize)]
struct SolveRequest<'a> {
    board: &'a Board,
}

#[derive(Deserialize)]
struct PuzzleResponse {
    board: Board,
}

#[derive(Deserialize)]
struct ValidateResponse {
    result: ValidationResult,
}

#[derive(Deserialize)]
struct SolveResponse {
    board: Option<Board>,
}

// ─── State ────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum Message {
    None,
    Success(String),
    Error(String),
    Info(String),
}

#[derive(Clone, PartialEq)]
struct AppState {
    board: Option<Board>,
    original: Option<Board>,
    selected: Option<usize>,
    error_cells: Vec<usize>,
    difficulty: Difficulty,
    loading: bool,
    elapsed_secs: u32,
    game_won: bool,
    message: Message,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            board: None,
            original: None,
            selected: None,
            error_cells: vec![],
            difficulty: Difficulty::Easy,
            loading: false,
            elapsed_secs: 0,
            game_won: false,
            message: Message::None,
        }
    }
}

// ─── Actions ─────────────────────────────────────────────────────────────────

enum Action {
    SetLoading(bool),
    SetBoard(Board),
    SelectCell(usize),
    EnterDigit(u8),
    SetErrors(Vec<usize>),
    SetWon,
    SetSolved(Board),
    SetMessage(Message),
    SetDifficulty(Difficulty),
    Tick,
    Reset,
}

impl Reducible for AppState {
    type Action = Action;

    fn reduce(self: std::rc::Rc<Self>, action: Action) -> std::rc::Rc<Self> {
        let mut state = (*self).clone();
        match action {
            Action::SetLoading(v) => {
                state.loading = v;
            }
            Action::SetBoard(board) => {
                state.board = Some(board.clone());
                state.original = Some(board);
                state.selected = None;
                state.error_cells = vec![];
                state.loading = false;
                state.elapsed_secs = 0;
                state.game_won = false;
                state.message = Message::Info("New game started!".into());
            }
            Action::SelectCell(idx) => {
                if let Some(ref board) = state.board {
                    if !board.givens[idx] && !state.game_won {
                        state.selected = Some(idx);
                    }
                }
            }
            Action::EnterDigit(digit) => {
                if let (Some(ref mut board), Some(idx)) = (&mut state.board, state.selected) {
                    if !board.givens[idx] {
                        board.cells[idx] = digit;
                        state.message = Message::None;
                    }
                }
            }
            Action::SetErrors(cells) => {
                state.error_cells = cells;
                if !state.error_cells.is_empty() {
                    state.message = Message::Error("There are errors on the board.".into());
                } else {
                    state.message = Message::Info("Looking good so far!".into());
                }
            }
            Action::SetWon => {
                state.game_won = true;
                state.error_cells = vec![];
                state.message = Message::Success(format!(
                    "Congratulations! Solved in {}!",
                    format_time(state.elapsed_secs)
                ));
            }
            Action::SetSolved(board) => {
                state.board = Some(board);
                state.error_cells = vec![];
                state.game_won = true;
                state.message = Message::Info("Board solved for you.".into());
            }
            Action::SetMessage(msg) => {
                state.message = msg;
            }
            Action::SetDifficulty(diff) => {
                state.difficulty = diff;
            }
            Action::Tick => {
                if !state.game_won && state.board.is_some() && !state.loading {
                    state.elapsed_secs += 1;
                }
            }
            Action::Reset => {
                if let (Some(ref mut board), Some(ref original)) =
                    (&mut state.board, &state.original)
                {
                    for i in 0..81 {
                        if !original.givens[i] {
                            board.cells[i] = 0;
                        }
                    }
                    state.error_cells = vec![];
                    state.selected = None;
                    state.message = Message::Info("Board reset.".into());
                }
            }
        }
        state.into()
    }
}

// ─── Root App component ───────────────────────────────────────────────────────

#[function_component(App)]
pub fn app() -> Html {
    let state = use_reducer(AppState::default);

    // Timer tick every second
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(1000, move || {
                state.dispatch(Action::Tick);
            });
            Box::leak(Box::new(interval));
            || ()
        });
    }

    // Fetch new puzzle
    let fetch_puzzle = {
        let state = state.clone();
        Callback::from(move |difficulty: Difficulty| {
            let state = state.clone();
            state.dispatch(Action::SetLoading(true));
            let diff_str = match difficulty {
                Difficulty::Easy => "easy",
                Difficulty::Medium => "medium",
                Difficulty::Hard => "hard",
            };
            let url = format!("/api/puzzle?difficulty={}", diff_str);
            wasm_bindgen_futures::spawn_local(async move {
                match Request::get(&url).send().await {
                    Ok(resp) => {
                        if let Ok(data) = resp.json::<PuzzleResponse>().await {
                            state.dispatch(Action::SetBoard(data.board));
                        } else {
                            state.dispatch(Action::SetMessage(Message::Error(
                                "Failed to parse puzzle.".into(),
                            )));
                            state.dispatch(Action::SetLoading(false));
                        }
                    }
                    Err(_) => {
                        state.dispatch(Action::SetMessage(Message::Error(
                            "Network error fetching puzzle.".into(),
                        )));
                        state.dispatch(Action::SetLoading(false));
                    }
                }
            });
        })
    };

    // Validate board
    let validate_board = {
        let state = state.clone();
        Callback::from(move |_: ()| {
            let state = state.clone();
            if let Some(ref board) = state.board {
                let body = serde_json::to_string(&ValidateRequest { board }).unwrap();
                let state2 = state.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::post("/api/validate")
                        .header("Content-Type", "application/json")
                        .body(body)
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(resp) => {
                            if let Ok(data) = resp.json::<ValidateResponse>().await {
                                if data.result.is_complete {
                                    state2.dispatch(Action::SetWon);
                                } else {
                                    state2.dispatch(Action::SetErrors(data.result.error_cells));
                                }
                            }
                        }
                        Err(_) => {
                            state2.dispatch(Action::SetMessage(Message::Error(
                                "Validation request failed.".into(),
                            )));
                        }
                    }
                });
            }
        })
    };

    // Solve board
    let solve_board = {
        let state = state.clone();
        Callback::from(move |_: ()| {
            let state = state.clone();
            if let Some(ref board) = state.board {
                let body = serde_json::to_string(&SolveRequest { board }).unwrap();
                let state2 = state.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::post("/api/solve")
                        .header("Content-Type", "application/json")
                        .body(body)
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(resp) => {
                            if let Ok(data) = resp.json::<SolveResponse>().await {
                                if let Some(solved) = data.board {
                                    state2.dispatch(Action::SetSolved(solved));
                                } else {
                                    state2.dispatch(Action::SetMessage(Message::Error(
                                        "No solution found.".into(),
                                    )));
                                }
                            }
                        }
                        Err(_) => {
                            state2.dispatch(Action::SetMessage(Message::Error(
                                "Solve request failed.".into(),
                            )));
                        }
                    }
                });
            }
        })
    };

    // Cell click
    let on_cell_click = {
        let state = state.clone();
        Callback::from(move |idx: usize| {
            state.dispatch(Action::SelectCell(idx));
        })
    };

    // Digit entry
    let on_digit = {
        let state = state.clone();
        Callback::from(move |digit: u8| {
            state.dispatch(Action::EnterDigit(digit));
        })
    };

    // Difficulty change
    let on_difficulty = {
        let state = state.clone();
        let fetch = fetch_puzzle.clone();
        Callback::from(move |diff: Difficulty| {
            state.dispatch(Action::SetDifficulty(diff));
            fetch.emit(diff);
        })
    };

    // New game
    let on_new_game = {
        let fetch = fetch_puzzle.clone();
        let difficulty = state.difficulty;
        Callback::from(move |_: MouseEvent| {
            fetch.emit(difficulty);
        })
    };

    // Reset
    let on_reset = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            state.dispatch(Action::Reset);
        })
    };

    // Validate
    let on_validate = {
        let validate = validate_board.clone();
        Callback::from(move |_: MouseEvent| {
            validate.emit(());
        })
    };

    // Solve
    let on_solve = {
        let solve = solve_board.clone();
        Callback::from(move |_: MouseEvent| {
            solve.emit(());
        })
    };

    html! {
        <div>
            <header>
                <h1>{"JCA's Sudoku"}</h1>
                <p>{"Built with Rust + Yew"}</p>
            </header>

            <div class="controls">
                <div class="difficulty-selector">
                    { for [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard].iter().map(|&d| {
                        let label = match d {
                            Difficulty::Easy   => "Easy",
                            Difficulty::Medium => "Medium",
                            Difficulty::Hard   => "Hard",
                        };
                        let active = state.difficulty == d;
                        let on_diff = on_difficulty.clone();
                        html! {
                            <button
                                class={classes!("diff-btn", active.then_some("active"))}
                                onclick={Callback::from(move |_| on_diff.emit(d))}
                            >
                                { label }
                            </button>
                        }
                    }) }
                </div>
                <button class="btn btn-primary" onclick={on_new_game} disabled={state.loading}>
                    { if state.loading { "Loading..." } else { "New Game" } }
                </button>
                <button class="btn btn-secondary" onclick={on_reset} disabled={state.board.is_none() || state.loading}>
                    {"Reset"}
                </button>
                <button class="btn btn-success" onclick={on_validate} disabled={state.board.is_none() || state.loading || state.game_won}>
                    {"Check"}
                </button>
                <button class="btn btn-warning" onclick={on_solve} disabled={state.board.is_none() || state.loading || state.game_won}>
                    {"Solve"}
                </button>
            </div>

            <div class="status-bar">
                if state.board.is_some() {
                    <span class="timer">{ format_time(state.elapsed_secs) }</span>
                }
                {
                    match &state.message {
                        Message::None => html!{},
                        Message::Success(m) => html!{ <span class="message success">{ m }</span> },
                        Message::Error(m)   => html!{ <span class="message error">{ m }</span> },
                        Message::Info(m)    => html!{ <span class="message info">{ m }</span> },
                    }
                }
            </div>

            if state.loading {
                <div class="loading">
                    <div class="spinner"></div>
                    <span>{"Generating puzzle…"}</span>
                </div>
            } else if let Some(ref board) = state.board {
                <div class="board-wrapper">
                    <BoardView
                        board={board.clone()}
                        selected={state.selected}
                        error_cells={state.error_cells.clone()}
                        on_cell_click={on_cell_click.clone()}
                    />
                </div>
                <NumPad on_digit={on_digit.clone()} />
            } else {
                <div class="loading">
                    <span>{"Press \"New Game\" to start!"}</span>
                </div>
            }
        </div>
    }
}

// ─── BoardView component ──────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
struct BoardProps {
    board: Board,
    selected: Option<usize>,
    error_cells: Vec<usize>,
    on_cell_click: Callback<usize>,
}

#[function_component(BoardView)]
fn board_view(props: &BoardProps) -> Html {
    let sel_row = props.selected.map(|i| i / 9);
    let sel_col = props.selected.map(|i| i % 9);
    let sel_box = props.selected.map(|i| (i / 9 / 3) * 3 + (i % 9 / 3));

    html! {
        <div class="board">
            { for (0..81usize).map(|idx| {
                let row = idx / 9;
                let col = idx % 9;
                let box_idx = (row / 3) * 3 + (col / 3);
                let val = props.board.cells[idx];
                let is_given = props.board.givens[idx];
                let is_selected = props.selected == Some(idx);
                let is_highlighted = !is_selected && (
                    sel_row == Some(row) ||
                    sel_col == Some(col) ||
                    sel_box == Some(box_idx)
                );
                let is_error = props.error_cells.contains(&idx);

                let cell_class = classes!(
                    "cell",
                    is_given.then_some("given"),
                    (!is_given && val != 0).then_some("user-input"),
                    (!is_given && val == 0).then_some("empty"),
                    is_selected.then_some("selected"),
                    is_highlighted.then_some("highlighted"),
                    is_error.then_some("error"),
                );

                let on_click = {
                    let cb = props.on_cell_click.clone();
                    Callback::from(move |_: MouseEvent| cb.emit(idx))
                };

                html! {
                    <div
                        class={cell_class}
                        data-row={row.to_string()}
                        data-col={col.to_string()}
                        onclick={on_click}
                    >
                        if val != 0 {
                            { val.to_string() }
                        }
                    </div>
                }
            }) }
        </div>
    }
}

// ─── NumPad component ─────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
struct NumPadProps {
    on_digit: Callback<u8>,
}

#[function_component(NumPad)]
fn num_pad(props: &NumPadProps) -> Html {
    let on_erase = {
        let cb = props.on_digit.clone();
        Callback::from(move |_: MouseEvent| cb.emit(0))
    };

    html! {
        <div class="numpad">
            { for (1u8..=9).map(|d| {
                let cb = props.on_digit.clone();
                html! {
                    <button class="numpad-btn" onclick={Callback::from(move |_: MouseEvent| cb.emit(d))}>
                        { d.to_string() }
                    </button>
                }
            }) }
            <button class="numpad-btn erase" onclick={on_erase}>
                {"Erase"}
            </button>
        </div>
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn format_time(secs: u32) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() {
    yew::Renderer::<App>::new().render();
}
