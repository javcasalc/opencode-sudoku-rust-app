//! sudoku-core: board representation, puzzle generation, solving, and validation.

use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use serde::{Deserialize, Serialize};

// ─── Types ───────────────────────────────────────────────────────────────────

/// A single cell value: 1-9, or 0 meaning empty.
pub type CellValue = u8;

/// 9×9 board stored as a flat array of 81 cells, row-major order.
/// Index = row * 9 + col. Value 0 = empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// Current cell values (0 = empty, 1-9 = digit).
    pub cells: [CellValue; 81],
    /// Mask: true if the cell is a "given" (part of the original puzzle, not editable).
    pub givens: [bool; 81],
}

impl Default for Board {
    fn default() -> Self {
        Board {
            cells: [0; 81],
            givens: [false; 81],
        }
    }
}

impl Board {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn get(&self, row: usize, col: usize) -> CellValue {
        self.cells[row * 9 + col]
    }

    #[inline]
    pub fn set(&mut self, row: usize, col: usize, val: CellValue) {
        self.cells[row * 9 + col] = val;
    }

    #[inline]
    pub fn is_given(&self, row: usize, col: usize) -> bool {
        self.givens[row * 9 + col]
    }
}

// ─── Difficulty ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    /// Number of cells to *remove* from a fully solved board.
    fn cells_to_remove(self) -> usize {
        match self {
            Difficulty::Easy => 36,
            Difficulty::Medium => 46,
            Difficulty::Hard => 52,
        }
    }
}

impl std::str::FromStr for Difficulty {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "easy" => Ok(Difficulty::Easy),
            "medium" => Ok(Difficulty::Medium),
            "hard" => Ok(Difficulty::Hard),
            other => Err(format!("unknown difficulty: {}", other)),
        }
    }
}

// ─── Generator ───────────────────────────────────────────────────────────────

/// Generate a new Sudoku puzzle with the given difficulty.
/// Uses a random seed for variety.
pub fn generate(difficulty: Difficulty) -> Board {
    let seed = rand::random::<u64>();
    generate_seeded(difficulty, seed)
}

/// Generate a puzzle with an explicit seed (useful for tests).
pub fn generate_seeded(difficulty: Difficulty, seed: u64) -> Board {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut solution = Board::new();
    fill_board(&mut solution, &mut rng);

    // Clone the solution, then remove cells.
    let mut puzzle = solution.clone();
    let to_remove = difficulty.cells_to_remove();
    remove_cells(&mut puzzle, to_remove, &mut rng);

    // Mark remaining non-zero cells as givens.
    for i in 0..81 {
        puzzle.givens[i] = puzzle.cells[i] != 0;
    }

    puzzle
}

/// Recursively fill a board using backtracking with shuffled candidates.
fn fill_board(board: &mut Board, rng: &mut SmallRng) -> bool {
    // Find the first empty cell.
    let pos = match (0..81).find(|&i| board.cells[i] == 0) {
        Some(p) => p,
        None => return true, // Board is complete.
    };

    let row = pos / 9;
    let col = pos % 9;

    let mut candidates: Vec<CellValue> = (1..=9).collect();
    candidates.shuffle(rng);

    for &val in &candidates {
        if is_valid_placement(board, row, col, val) {
            board.cells[pos] = val;
            if fill_board(board, rng) {
                return true;
            }
            board.cells[pos] = 0;
        }
    }
    false
}

/// Remove `count` cells from a fully solved board, ensuring a solution still exists.
/// For simplicity we remove cells randomly; for Medium/Hard we don't re-check
/// uniqueness (acceptable for a game).
fn remove_cells(board: &mut Board, count: usize, rng: &mut SmallRng) {
    let mut indices: Vec<usize> = (0..81).collect();
    indices.shuffle(rng);

    let mut removed = 0;
    for idx in indices {
        if removed >= count {
            break;
        }
        let backup = board.cells[idx];
        board.cells[idx] = 0;

        // Quick check: does the puzzle still have at least one solution?
        let mut tmp = board.clone();
        if solve_internal(&mut tmp) {
            removed += 1;
        } else {
            // Restore if it broke solvability.
            board.cells[idx] = backup;
        }
    }
}

// ─── Solver ──────────────────────────────────────────────────────────────────

/// Solve the puzzle in-place. Returns `true` if a solution was found.
pub fn solve(board: &mut Board) -> bool {
    solve_internal(board)
}

fn solve_internal(board: &mut Board) -> bool {
    // Find the empty cell with fewest candidates (MRV heuristic).
    let pos = match best_empty_cell(board) {
        Some(p) => p,
        None => return true, // All cells filled — solved.
    };

    let row = pos / 9;
    let col = pos % 9;

    for val in 1u8..=9 {
        if is_valid_placement(board, row, col, val) {
            board.cells[pos] = val;
            if solve_internal(board) {
                return true;
            }
            board.cells[pos] = 0;
        }
    }
    false
}

/// Returns the index of the empty cell with the minimum number of legal candidates.
fn best_empty_cell(board: &Board) -> Option<usize> {
    let mut best_pos = None;
    let mut best_count = 10usize;

    for i in 0..81 {
        if board.cells[i] == 0 {
            let count = candidates(board, i / 9, i % 9).len();
            if count == 0 {
                return Some(i); // Dead-end — return it so backtracking kicks in.
            }
            if count < best_count {
                best_count = count;
                best_pos = Some(i);
            }
        }
    }
    best_pos
}

/// Returns the list of valid digits for a given cell.
fn candidates(board: &Board, row: usize, col: usize) -> Vec<CellValue> {
    (1u8..=9)
        .filter(|&v| is_valid_placement(board, row, col, v))
        .collect()
}

// ─── Validator ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationResult {
    /// True if every cell is filled AND there are no rule violations.
    pub is_complete: bool,
    /// True if any rule is violated (duplicate in row/col/box).
    pub has_errors: bool,
    /// Flat indices (0-80) of cells that participate in a violation.
    pub error_cells: Vec<usize>,
}

pub fn validate(board: &Board) -> ValidationResult {
    let mut error_set = std::collections::HashSet::new();

    // Check rows.
    for row in 0..9 {
        check_group(board, (0..9).map(|col| row * 9 + col), &mut error_set);
    }
    // Check columns.
    for col in 0..9 {
        check_group(board, (0..9).map(|row| row * 9 + col), &mut error_set);
    }
    // Check 3×3 boxes.
    for box_row in 0..3 {
        for box_col in 0..3 {
            let indices = (0..3).flat_map(|r| {
                (0..3).map(move |c| (box_row * 3 + r) * 9 + (box_col * 3 + c))
            });
            check_group(board, indices, &mut error_set);
        }
    }

    let has_errors = !error_set.is_empty();
    let is_complete = !has_errors && board.cells.iter().all(|&v| v != 0);
    let mut error_cells: Vec<usize> = error_set.into_iter().collect();
    error_cells.sort_unstable();

    ValidationResult {
        is_complete,
        has_errors,
        error_cells,
    }
}

/// Check a group (row/col/box) and add duplicate cell indices to the error set.
fn check_group(
    board: &Board,
    indices: impl Iterator<Item = usize>,
    errors: &mut std::collections::HashSet<usize>,
) {
    let mut seen: [Option<usize>; 10] = [None; 10]; // seen[digit] = first index
    let idx_vec: Vec<usize> = indices.collect();

    for &idx in &idx_vec {
        let val = board.cells[idx] as usize;
        if val == 0 {
            continue;
        }
        if let Some(prev) = seen[val] {
            errors.insert(prev);
            errors.insert(idx);
        } else {
            seen[val] = Some(idx);
        }
    }
}

// ─── Placement validity ──────────────────────────────────────────────────────

/// Returns true if placing `val` at (row, col) violates no Sudoku rules.
pub fn is_valid_placement(board: &Board, row: usize, col: usize, val: CellValue) -> bool {
    // Row check.
    for c in 0..9 {
        if board.get(row, c) == val {
            return false;
        }
    }
    // Column check.
    for r in 0..9 {
        if board.get(r, col) == val {
            return false;
        }
    }
    // 3×3 box check.
    let box_row = (row / 3) * 3;
    let box_col = (col / 3) * 3;
    for r in box_row..box_row + 3 {
        for c in box_col..box_col + 3 {
            if board.get(r, c) == val {
                return false;
            }
        }
    }
    true
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn solved_board() -> Board {
        // A known valid solved Sudoku board.
        #[rustfmt::skip]
        let cells: [u8; 81] = [
            5,3,4, 6,7,8, 9,1,2,
            6,7,2, 1,9,5, 3,4,8,
            1,9,8, 3,4,2, 5,6,7,
            8,5,9, 7,6,1, 4,2,3,
            4,2,6, 8,5,3, 7,9,1,
            7,1,3, 9,2,4, 8,5,6,
            9,6,1, 5,3,7, 2,8,4,
            2,8,7, 4,1,9, 6,3,5,
            3,4,5, 2,8,6, 1,7,9,
        ];
        Board { cells, givens: [true; 81] }
    }

    #[test]
    fn test_validate_solved_board() {
        let board = solved_board();
        let result = validate(&board);
        assert!(result.is_complete, "Solved board should be complete");
        assert!(!result.has_errors, "Solved board should have no errors");
        assert!(result.error_cells.is_empty());
    }

    #[test]
    fn test_validate_detects_row_duplicate() {
        let mut board = solved_board();
        // Introduce a duplicate in row 0: set cell (0,1) to same as (0,0) = 5.
        board.cells[1] = 5;
        let result = validate(&board);
        assert!(result.has_errors);
        assert!(result.error_cells.contains(&0));
        assert!(result.error_cells.contains(&1));
    }

    #[test]
    fn test_validate_detects_col_duplicate() {
        let mut board = solved_board();
        // Set (1,0) = 5, same as (0,0) — duplicate in column 0.
        board.cells[9] = 5;
        let result = validate(&board);
        assert!(result.has_errors);
    }

    #[test]
    fn test_validate_incomplete_board() {
        let mut board = solved_board();
        board.cells[0] = 0; // Remove one cell.
        let result = validate(&board);
        assert!(!result.is_complete);
        assert!(!result.has_errors);
    }

    #[test]
    fn test_solver_solves_easy_puzzle() {
        let mut puzzle = generate_seeded(Difficulty::Easy, 42);
        let empty_before = puzzle.cells.iter().filter(|&&v| v == 0).count();
        assert!(empty_before > 0, "Puzzle should have empty cells");
        let solved = solve(&mut puzzle);
        assert!(solved, "Solver should find a solution");
        let result = validate(&puzzle);
        assert!(result.is_complete, "Solved puzzle should be complete");
        assert!(!result.has_errors, "Solved puzzle should have no errors");
    }

    #[test]
    fn test_solver_solves_hard_puzzle() {
        let mut puzzle = generate_seeded(Difficulty::Hard, 99);
        let solved = solve(&mut puzzle);
        assert!(solved, "Solver should find a solution for hard puzzle");
        let result = validate(&puzzle);
        assert!(result.is_complete);
        assert!(!result.has_errors);
    }

    #[test]
    fn test_generate_easy_has_correct_givens() {
        let board = generate_seeded(Difficulty::Easy, 1);
        let given_count = board.givens.iter().filter(|&&g| g).count();
        // Easy removes 36 cells → 45 givens remain.
        assert_eq!(given_count, 81 - Difficulty::Easy.cells_to_remove());
    }

    #[test]
    fn test_generate_hard_has_fewer_givens_than_easy() {
        let easy = generate_seeded(Difficulty::Easy, 7);
        let hard = generate_seeded(Difficulty::Hard, 7);
        let easy_givens = easy.givens.iter().filter(|&&g| g).count();
        let hard_givens = hard.givens.iter().filter(|&&g| g).count();
        assert!(hard_givens <= easy_givens);
    }

    #[test]
    fn test_is_valid_placement() {
        let board = Board::new(); // All empty.
        assert!(is_valid_placement(&board, 0, 0, 5));
        let mut b2 = Board::new();
        b2.set(0, 3, 5);
        assert!(!is_valid_placement(&b2, 0, 0, 5)); // Same row.
        assert!(!is_valid_placement(&b2, 4, 3, 5)); // Same col.
        assert!(!is_valid_placement(&b2, 2, 4, 5)); // Same box.
    }

    #[test]
    fn test_difficulty_from_str() {
        assert_eq!("easy".parse::<Difficulty>().unwrap(), Difficulty::Easy);
        assert_eq!("MEDIUM".parse::<Difficulty>().unwrap(), Difficulty::Medium);
        assert_eq!("hard".parse::<Difficulty>().unwrap(), Difficulty::Hard);
        assert!("extreme".parse::<Difficulty>().is_err());
    }
}
