//! Calculates tmux pane grid layouts for agent terminals.
//! Pure functions — no I/O.

/// A cell in the tmux pane grid.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub struct GridCell {
    /// Row index (0-based)
    pub row: usize,
    /// Column index (0-based)
    pub col: usize,
}

/// The computed grid layout.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GridLayout {
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub cols: usize,
}

/// Calculate grid dimensions for the given number of workers.
///
/// Strategy:
/// - 1 worker: 1×1 (single pane)
/// - 2 workers: 1×2 (side by side)
/// - 3-4 workers: 2×2
/// - 5-8 workers: 2×4 (2 rows, 4 cols)
/// - 9-12 workers: 3×4
/// - 13+ workers: 4×4
#[allow(dead_code)]
pub fn calculate_layout(worker_count: usize) -> GridLayout {
    let (rows, cols) = match worker_count {
        0 => (1, 1),
        1 => (1, 1),
        2 => (1, 2),
        3..=4 => (2, 2),
        5..=8 => (2, 4),
        9..=12 => (3, 4),
        _ => (4, 4),
    };
    GridLayout { rows, cols }
}

/// Given a layout, compute the cell position for each worker index.
/// Workers fill row-by-row, left to right.
#[cfg(test)]
pub fn worker_cells(layout: &GridLayout) -> Vec<GridCell> {
    (0..layout.rows)
        .flat_map(|r| (0..layout.cols).map(move |c| GridCell { row: r, col: c }))
        .take(layout.rows * layout.cols)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_1_worker() {
        let g = calculate_layout(1);
        assert_eq!(g.rows, 1);
        assert_eq!(g.cols, 1);
    }

    #[test]
    fn layout_2_workers() {
        let g = calculate_layout(2);
        assert_eq!(g.rows, 1);
        assert_eq!(g.cols, 2);
    }

    #[test]
    fn layout_4_workers() {
        let g = calculate_layout(4);
        assert_eq!(g.rows, 2);
        assert_eq!(g.cols, 2);
    }

    #[test]
    fn layout_8_workers() {
        let g = calculate_layout(8);
        assert_eq!(g.rows, 2);
        assert_eq!(g.cols, 4);
    }

    #[test]
    fn cells_fill_row_major() {
        let g = calculate_layout(2);
        let cells = worker_cells(&g);
        assert_eq!(cells[0].row, 0);
        assert_eq!(cells[0].col, 0);
        assert_eq!(cells[1].row, 0);
        assert_eq!(cells[1].col, 1);
    }

    #[test]
    fn cells_2x2() {
        let g = calculate_layout(4);
        let cells = worker_cells(&g);
        assert_eq!(cells[0], GridCell { row: 0, col: 0 });
        assert_eq!(cells[1], GridCell { row: 0, col: 1 });
        assert_eq!(cells[2], GridCell { row: 1, col: 0 });
        assert_eq!(cells[3], GridCell { row: 1, col: 1 });
    }
}
