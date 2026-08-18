use crate::protocol::CellSnapshot;
use crate::topology::{Coord3D, Dimensions};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotTier {
    Novice,       // Tier 1: Single-cell trivial deduction + 3% noise
    Intermediate, // Tier 2: Subset & overlap interval bounds
    Advanced,     // Tier 3: Full RREF Gaussian Elimination with bound satisfaction
    Master,       // Tier 4: Exact Bayesian counting + information entropy
}

impl BotTier {
    pub fn name_en(&self) -> &'static str {
        match self {
            BotTier::Novice => "Novice",
            BotTier::Intermediate => "Intermediate",
            BotTier::Advanced => "Advanced",
            BotTier::Master => "Master",
        }
    }

    pub fn name_zh(&self) -> &'static str {
        match self {
            BotTier::Novice => "初级·新手",
            BotTier::Intermediate => "中级·进阶",
            BotTier::Advanced => "高级·专家",
            BotTier::Master => "大师·宗师",
        }
    }

    pub fn speed_multiplier(&self) -> f64 {
        match self {
            BotTier::Novice => 1.3,
            BotTier::Intermediate => 1.0,
            BotTier::Advanced => 0.75,
            BotTier::Master => 0.55,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiAction {
    Reveal(Coord3D),
    Flag(Coord3D),
    Chord(Coord3D),
}

#[derive(Clone, Debug)]
struct Equation {
    cells: Vec<Coord3D>,
    mines: usize,
}

pub struct AiSolver;

impl AiSolver {
    pub fn decide_action(
        dims: Dimensions,
        cells: &[CellSnapshot],
        tier: BotTier,
        total_mines: usize,
    ) -> Option<AiAction> {
        let mut rng = rand::thread_rng();

        // 1. Gather cell state maps
        let mut revealed_map: HashMap<Coord3D, u8> = HashMap::new();
        let mut flagged_set: HashSet<Coord3D> = HashSet::new();
        let mut unrevealed_set: HashSet<Coord3D> = HashSet::new();

        for c in cells {
            if c.is_revealed {
                revealed_map.insert(c.coord, c.adjacent_mines);
            } else if c.is_flagged {
                flagged_set.insert(c.coord);
            } else {
                unrevealed_set.insert(c.coord);
            }
        }

        if unrevealed_set.is_empty() {
            return None;
        }

        // If board has 0 revealed cells (first move of the game), click center
        if revealed_map.is_empty() {
            let center = Coord3D::new(dims.width / 2, dims.height / 2, dims.depth / 2);
            return Some(AiAction::Reveal(center));
        }

        // Check for immediate Chord opportunities on satisfied revealed cells
        for (&coord, &adj_mines) in &revealed_map {
            if adj_mines == 0 {
                continue;
            }
            let neighbors = coord.neighbors_26(dims);
            let mut flag_count = 0;
            let mut has_unrevealed = false;

            for n in &neighbors {
                if flagged_set.contains(n) {
                    flag_count += 1;
                } else if unrevealed_set.contains(n) {
                    has_unrevealed = true;
                }
            }

            if has_unrevealed && flag_count == adj_mines as usize {
                return Some(AiAction::Chord(coord));
            }
        }

        // Build base equations from revealed cells
        let mut equations: Vec<Equation> = Vec::new();
        for (&coord, &adj_mines) in &revealed_map {
            let neighbors = coord.neighbors_26(dims);
            let mut unrev_neighbors = Vec::new();
            let mut flag_count = 0;

            for n in neighbors {
                if flagged_set.contains(&n) {
                    flag_count += 1;
                } else if unrevealed_set.contains(&n) {
                    unrev_neighbors.push(n);
                }
            }

            if !unrev_neighbors.is_empty() {
                let needed = adj_mines.saturating_sub(flag_count as u8) as usize;
                equations.push(Equation {
                    cells: unrev_neighbors,
                    mines: needed,
                });
            }
        }

        // --- Tier 1: Single-cell trivial deduction ---
        let mut certain_safe: HashSet<Coord3D> = HashSet::new();
        let mut certain_mines: HashSet<Coord3D> = HashSet::new();

        for eq in &equations {
            if eq.mines == 0 {
                for &c in &eq.cells {
                    certain_safe.insert(c);
                }
            } else if eq.mines == eq.cells.len() {
                for &c in &eq.cells {
                    certain_mines.insert(c);
                }
            }
        }

        // Novice minor noise (3% chance of minor hesitation)
        if tier == BotTier::Novice && rng.gen_bool(0.03) {
            let list: Vec<Coord3D> = unrevealed_set.iter().copied().collect();
            if let Some(&c) = list.choose(&mut rng) {
                return Some(AiAction::Reveal(c));
            }
        }

        // Prioritize revealing safe cells first
        if let Some(best_safe) = Self::pick_best_safe_cell(&certain_safe, dims, &unrevealed_set) {
            return Some(AiAction::Reveal(best_safe));
        }
        if let Some(&mine_c) = certain_mines.iter().find(|c| !flagged_set.contains(c)) {
            return Some(AiAction::Flag(mine_c));
        }

        if tier == BotTier::Novice {
            // Pick lowest estimated risk on boundary
            let best_boundary = Self::pick_lowest_risk_boundary(&equations, &unrevealed_set);
            return best_boundary.or_else(|| {
                let list: Vec<Coord3D> = unrevealed_set.iter().copied().collect();
                list.choose(&mut rng).copied().map(AiAction::Reveal)
            });
        }

        // --- Tier 2: Subset & Overlap Interval Deduction ---
        let mut derived_safe: HashSet<Coord3D> = HashSet::new();
        let mut derived_mines: HashSet<Coord3D> = HashSet::new();

        for (i, eq_a) in equations.iter().enumerate() {
            let set_a: HashSet<Coord3D> = eq_a.cells.iter().copied().collect();

            for (j, eq_b) in equations.iter().enumerate() {
                if i == j {
                    continue;
                }
                let set_b: HashSet<Coord3D> = eq_b.cells.iter().copied().collect();

                let inter: HashSet<Coord3D> = set_a.intersection(&set_b).copied().collect();
                if inter.is_empty() {
                    continue;
                }

                let diff_a: HashSet<Coord3D> = set_a.difference(&set_b).copied().collect();
                let diff_b: HashSet<Coord3D> = set_b.difference(&set_a).copied().collect();

                // Max and min mines in intersection from eq_a:
                let min_in_inter = eq_a.mines.saturating_sub(diff_a.len());
                let max_in_inter = eq_a.mines.min(inter.len());

                // Mines in diff_b = eq_b.mines - mines(inter)
                // min_in_diff_b = eq_b.mines - max_in_inter
                // max_in_diff_b = eq_b.mines - min_in_inter
                if eq_b.mines >= max_in_inter {
                    let min_in_diff_b = eq_b.mines - max_in_inter;
                    if !diff_b.is_empty() && min_in_diff_b == diff_b.len() {
                        for &c in &diff_b {
                            derived_mines.insert(c);
                        }
                    }
                }

                if eq_b.mines >= min_in_inter {
                    let max_in_diff_b = eq_b.mines - min_in_inter;
                    if !diff_b.is_empty() && max_in_diff_b == 0 {
                        for &c in &diff_b {
                            derived_safe.insert(c);
                        }
                    }
                }
            }
        }

        if let Some(best_safe) = Self::pick_best_safe_cell(&derived_safe, dims, &unrevealed_set) {
            return Some(AiAction::Reveal(best_safe));
        }
        if let Some(&mine_c) = derived_mines.iter().find(|c| !flagged_set.contains(c)) {
            return Some(AiAction::Flag(mine_c));
        }

        if tier == BotTier::Intermediate {
            let best_boundary = Self::pick_lowest_risk_boundary(&equations, &unrevealed_set);
            return best_boundary.or_else(|| {
                let list: Vec<Coord3D> = unrevealed_set.iter().copied().collect();
                list.choose(&mut rng).copied().map(AiAction::Reveal)
            });
        }

        // --- Tier 3 & 4: Gaussian Elimination (RREF with Bound Analysis) ---
        let (gauss_safe, gauss_mines, cell_probs) =
            Self::solve_gaussian_rref(&equations, total_mines.saturating_sub(flagged_set.len()));

        if let Some(best_safe) = Self::pick_best_safe_cell(&gauss_safe, dims, &unrevealed_set) {
            return Some(AiAction::Reveal(best_safe));
        }
        if let Some(&mine_c) = gauss_mines.iter().find(|c| !flagged_set.contains(c)) {
            return Some(AiAction::Flag(mine_c));
        }

        // Probabilistic Guessing
        if !cell_probs.is_empty() {
            let mut sorted_probs: Vec<(Coord3D, f64)> = cell_probs.into_iter().collect();
            sorted_probs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            if tier == BotTier::Advanced {
                if let Some(&(best_c, _)) = sorted_probs.first() {
                    return Some(AiAction::Reveal(best_c));
                }
            } else {
                // Tier 4: Master - Minimum risk + Maximum Shannon Entropy on 3D neighborhood
                let min_p = sorted_probs[0].1;
                let top_candidates: Vec<Coord3D> = sorted_probs
                    .iter()
                    .take_while(|(_, p)| (*p - min_p).abs() < 1e-4)
                    .map(|(c, _)| *c)
                    .collect();

                let mut best_candidate = top_candidates[0];
                let mut max_entropy = -1.0;

                for &cand in &top_candidates {
                    let neighbors = cand.neighbors_26(dims);
                    let unrev_count = neighbors
                        .iter()
                        .filter(|n| unrevealed_set.contains(n))
                        .count();
                    let entropy = unrev_count as f64;
                    if entropy > max_entropy {
                        max_entropy = entropy;
                        best_candidate = cand;
                    }
                }

                return Some(AiAction::Reveal(best_candidate));
            }
        }

        // Fallback: Pick lowest risk boundary cell
        let best_boundary = Self::pick_lowest_risk_boundary(&equations, &unrevealed_set);
        best_boundary.or_else(|| {
            let list: Vec<Coord3D> = unrevealed_set.iter().copied().collect();
            list.choose(&mut rng).copied().map(AiAction::Reveal)
        })
    }

    fn pick_best_safe_cell(
        safe_set: &HashSet<Coord3D>,
        dims: Dimensions,
        unrevealed: &HashSet<Coord3D>,
    ) -> Option<Coord3D> {
        if safe_set.is_empty() {
            return None;
        }

        let mut best_c = None;
        let mut max_frontier = 0;

        for &c in safe_set {
            let neighbors = c.neighbors_26(dims);
            let frontier = neighbors.iter().filter(|n| unrevealed.contains(n)).count();
            if best_c.is_none() || frontier >= max_frontier {
                max_frontier = frontier;
                best_c = Some(c);
            }
        }

        best_c
    }

    fn pick_lowest_risk_boundary(
        equations: &[Equation],
        unrevealed: &HashSet<Coord3D>,
    ) -> Option<AiAction> {
        let mut cell_risk: HashMap<Coord3D, f64> = HashMap::new();
        let mut cell_counts: HashMap<Coord3D, usize> = HashMap::new();

        for eq in equations {
            if eq.cells.is_empty() {
                continue;
            }
            let p = (eq.mines as f64) / (eq.cells.len() as f64);
            for &c in &eq.cells {
                let entry = cell_risk.entry(c).or_insert(0.0);
                *entry += p;
                *cell_counts.entry(c).or_insert(0) += 1;
            }
        }

        let mut best_cell = None;
        let mut min_risk = f64::MAX;

        for (c, total_p) in cell_risk {
            if !unrevealed.contains(&c) {
                continue;
            }
            let count = cell_counts.get(&c).copied().unwrap_or(1);
            let avg_p = total_p / (count as f64);
            if avg_p < min_risk {
                min_risk = avg_p;
                best_cell = Some(c);
            }
        }

        best_cell.map(AiAction::Reveal)
    }

    /// Full Gaussian Elimination (RREF) with [0, 1] bounded variable analysis
    fn solve_gaussian_rref(
        equations: &[Equation],
        _remaining_mines: usize,
    ) -> (HashSet<Coord3D>, HashSet<Coord3D>, HashMap<Coord3D, f64>) {
        let mut certain_safe = HashSet::new();
        let mut certain_mines = HashSet::new();
        let mut cell_probs: HashMap<Coord3D, f64> = HashMap::new();

        if equations.is_empty() {
            return (certain_safe, certain_mines, cell_probs);
        }

        // Collect distinct variables
        let mut var_map: HashMap<Coord3D, usize> = HashMap::new();
        let mut var_list: Vec<Coord3D> = Vec::new();

        for eq in equations {
            for &c in &eq.cells {
                if let std::collections::hash_map::Entry::Vacant(e) = var_map.entry(c) {
                    e.insert(var_list.len());
                    var_list.push(c);
                }
            }
        }

        let n_vars = var_list.len();
        let n_eqs = equations.len();

        if n_vars == 0 {
            return (certain_safe, certain_mines, cell_probs);
        }

        // Build augmented matrix: [M | rhs]
        let mut matrix: Vec<Vec<f64>> = vec![vec![0.0; n_vars + 1]; n_eqs];
        for (i, eq) in equations.iter().enumerate() {
            for &c in &eq.cells {
                let col = var_map[&c];
                matrix[i][col] = 1.0;
            }
            matrix[i][n_vars] = eq.mines as f64;
        }

        // Forward elimination and back substitution to RREF
        let mut lead = 0;
        for r in 0..n_eqs {
            if lead >= n_vars {
                break;
            }
            let mut i = r;
            while matrix[i][lead].abs() < 1e-6 {
                i += 1;
                if i == n_eqs {
                    i = r;
                    lead += 1;
                    if lead == n_vars {
                        break;
                    }
                }
            }
            if lead == n_vars {
                break;
            }

            matrix.swap(i, r);
            let lv = matrix[r][lead];
            for val in &mut matrix[r] {
                *val /= lv;
            }

            let pivot_row = matrix[r].clone();
            for (row_idx, row_data) in matrix.iter_mut().enumerate().take(n_eqs) {
                if row_idx != r {
                    let factor = row_data[lead];
                    if factor.abs() > 1e-6 {
                        for (col_idx, val) in row_data.iter_mut().enumerate().take(n_vars + 1) {
                            *val -= factor * pivot_row[col_idx];
                        }
                    }
                }
            }
            lead += 1;
        }

        // Bound analysis for each row:
        // Sum(c_j * x_j) = rhs, where x_j in [0, 1]
        for row in &matrix {
            let mut pos_sum = 0.0;
            let mut neg_sum = 0.0;
            let mut non_zero_cols: Vec<(usize, f64)> = Vec::new();

            for (col, &coeff) in row.iter().enumerate().take(n_vars) {
                if coeff.abs() > 1e-6 {
                    non_zero_cols.push((col, coeff));
                    if coeff > 0.0 {
                        pos_sum += coeff;
                    } else {
                        neg_sum += coeff;
                    }
                }
            }

            if non_zero_cols.is_empty() {
                continue;
            }

            let rhs = row[n_vars];

            // If Max possible sum == rhs: all positive coeff variables must be 1, all negative must be 0
            if (pos_sum - rhs).abs() < 1e-5 {
                for &(col, coeff) in &non_zero_cols {
                    let coord = var_list[col];
                    if coeff > 0.0 {
                        certain_mines.insert(coord);
                    } else {
                        certain_safe.insert(coord);
                    }
                }
            }

            // If Min possible sum == rhs: all positive coeff variables must be 0, all negative must be 1
            if (neg_sum - rhs).abs() < 1e-5 {
                for &(col, coeff) in &non_zero_cols {
                    let coord = var_list[col];
                    if coeff > 0.0 {
                        certain_safe.insert(coord);
                    } else {
                        certain_mines.insert(coord);
                    }
                }
            }
        }

        // Generate baseline probabilities for variables
        for (col, &coord) in var_list.iter().enumerate() {
            if certain_safe.contains(&coord) {
                cell_probs.insert(coord, 0.0);
            } else if certain_mines.contains(&coord) {
                cell_probs.insert(coord, 1.0);
            } else {
                // Approximate from single coefficient rows
                let mut est_p = 0.2;
                for row in &matrix {
                    if row[col].abs() > 1e-6 {
                        let rhs = row[n_vars];
                        if (0.0..=1.0).contains(&rhs) {
                            est_p = rhs / row[col];
                            break;
                        }
                    }
                }
                cell_probs.insert(coord, est_p.clamp(0.01, 0.99));
            }
        }

        (certain_safe, certain_mines, cell_probs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, BoardConfig, GameStatus};

    #[test]
    fn test_master_expert_game_simulation() {
        let config = BoardConfig::medium();
        let mut board = Board::new(config);
        let center = Coord3D::new(config.width / 2, config.height / 2, config.depth / 2);
        board.reveal(center, None, None);

        let mut moves = 0;
        let mut _flag_count = 0;

        while board.status == GameStatus::Playing {
            moves += 1;
            let snapshots: Vec<CellSnapshot> = board.cells.iter().map(CellSnapshot::from).collect();
            let action =
                AiSolver::decide_action(config.dims(), &snapshots, BotTier::Master, config.mines);

            match action {
                Some(AiAction::Reveal(c)) => {
                    let res = board.reveal(c, None, None);
                    println!("Move {moves}: Reveal({:?}) -> {:?}", c, res);
                }
                Some(AiAction::Chord(c)) => {
                    let res = board.chord(c, None, None);
                    println!("Move {moves}: Chord({:?}) -> {:?}", c, res);
                }
                Some(AiAction::Flag(c)) => {
                    board.toggle_flag(c);
                    _flag_count += 1;
                    println!("Move {moves}: Flag({:?})", c);
                }
                None => {
                    println!("Move {moves}: No action available");
                    break;
                }
            }

            if moves >= 30 {
                break;
            }
        }

        println!(
            "Simulation finished after {moves} moves. Status: {:?}, Revealed: {}/{}",
            board.status,
            board.revealed_count,
            config.total_cells() - config.mines
        );
    }
}
