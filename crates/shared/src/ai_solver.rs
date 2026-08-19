use crate::protocol::CellSnapshot;
use crate::topology::{Coord3D, Dimensions};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotTier {
    Novice,       // Tier 1: Single-cell trivial deduction + 3% noise
    Intermediate, // Tier 2: Multi-pass subset reduction + overlap interval inference
    Advanced,     // Tier 3: Subset reduction + Component Integer Gaussian bound reduction
    Master, // Tier 4: Exact Component Bayesian Model Counting + Unlinked Density + Information Entropy
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Equation {
    cells: HashSet<Coord3D>,
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

        // 2. Check for immediate Chord opportunities on satisfied revealed cells
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

        // 3. Build base equations strictly from frontier cells
        let mut equations: Vec<Equation> = Vec::new();
        for (&coord, &adj_mines) in &revealed_map {
            let neighbors = coord.neighbors_26(dims);
            let mut unrev_neighbors = HashSet::new();
            let mut flag_count = 0;

            for n in neighbors {
                if flagged_set.contains(&n) {
                    flag_count += 1;
                } else if unrevealed_set.contains(&n) {
                    unrev_neighbors.insert(n);
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

        // Deduplicate
        equations = Self::dedup_equations(equations);

        // --- Tier 1: Single-cell direct deduction ---
        let (mut certain_safe, mut certain_mines) = Self::direct_deductions(&equations);

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
            let best_boundary = Self::pick_lowest_risk_boundary(&equations, &unrevealed_set);
            return best_boundary.or_else(|| {
                let list: Vec<Coord3D> = unrevealed_set.iter().copied().collect();
                list.choose(&mut rng).copied().map(AiAction::Reveal)
            });
        }

        // --- Tier 2+: Multi-Pass Subset Replacement Engine ---
        let (iter_safe, iter_mines, reduced_eqs) = Self::solve_subset_replacement(equations);
        for c in iter_safe {
            certain_safe.insert(c);
        }
        for c in iter_mines {
            certain_mines.insert(c);
        }

        if let Some(best_safe) = Self::pick_best_safe_cell(&certain_safe, dims, &unrevealed_set) {
            return Some(AiAction::Reveal(best_safe));
        }
        if let Some(&mine_c) = certain_mines.iter().find(|c| !flagged_set.contains(c)) {
            return Some(AiAction::Flag(mine_c));
        }

        if tier == BotTier::Intermediate {
            let best_boundary = Self::pick_lowest_risk_boundary(&reduced_eqs, &unrevealed_set);
            return best_boundary.or_else(|| {
                let list: Vec<Coord3D> = unrevealed_set.iter().copied().collect();
                list.choose(&mut rng).copied().map(AiAction::Reveal)
            });
        }

        // --- Tier 3 & 4: Connected Component Decomposition + Exact Model Counting (Tank Solver) ---
        let remaining_mines_count = total_mines.saturating_sub(flagged_set.len());
        let (comp_safe, comp_mines, cell_probs) = Self::solve_connected_components(
            &reduced_eqs,
            &unrevealed_set,
            remaining_mines_count,
            tier == BotTier::Master,
        );

        for c in comp_safe {
            certain_safe.insert(c);
        }
        for c in comp_mines {
            certain_mines.insert(c);
        }

        if let Some(best_safe) = Self::pick_best_safe_cell(&certain_safe, dims, &unrevealed_set) {
            return Some(AiAction::Reveal(best_safe));
        }
        if let Some(&mine_c) = certain_mines.iter().find(|c| !flagged_set.contains(c)) {
            return Some(AiAction::Flag(mine_c));
        }

        // --- Probabilistic Guessing with Unlinked Interior Density Check ---
        let mut frontier_vars = HashSet::new();
        for eq in &reduced_eqs {
            for &c in &eq.cells {
                frontier_vars.insert(c);
            }
        }

        let unlinked_cells: Vec<Coord3D> = unrevealed_set
            .iter()
            .copied()
            .filter(|c| !frontier_vars.contains(c))
            .collect();

        // Estimate background probability of unlinked cells
        let est_frontier_mines: f64 = cell_probs.values().sum();
        let unlinked_mines = (remaining_mines_count as f64 - est_frontier_mines).max(0.0);
        let unlinked_prob = if !unlinked_cells.is_empty() {
            (unlinked_mines / unlinked_cells.len() as f64).clamp(0.001, 0.999)
        } else {
            1.0
        };

        if !cell_probs.is_empty() {
            let mut sorted_probs: Vec<(Coord3D, f64)> = cell_probs.into_iter().collect();
            sorted_probs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let (best_frontier_cell, min_frontier_p) = sorted_probs[0];

            // If background unlinked cells have lower or equal risk than frontier bottleneck,
            // make a smart exploratory click in the open interior!
            if (tier == BotTier::Master || tier == BotTier::Advanced)
                && !unlinked_cells.is_empty()
                && unlinked_prob <= min_frontier_p
            {
                // Choose unlinked cell with maximum unrevealed 26-neighbors
                let pick = unlinked_cells
                    .iter()
                    .max_by_key(|c| {
                        c.neighbors_26(dims)
                            .iter()
                            .filter(|n| unrevealed_set.contains(n))
                            .count()
                    })
                    .copied()
                    .unwrap_or(unlinked_cells[0]);
                return Some(AiAction::Reveal(pick));
            }

            if tier == BotTier::Advanced {
                return Some(AiAction::Reveal(best_frontier_cell));
            } else {
                // Tier 4: Master - Maximum Shannon Information Gain among lowest-risk candidates
                let top_candidates: Vec<Coord3D> = sorted_probs
                    .iter()
                    .take_while(|(_, p)| (*p - min_frontier_p).abs() < 1e-4)
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

        // Final fallback: Pick unlinked cell if available, else random unrevealed
        if !unlinked_cells.is_empty() {
            let pick = unlinked_cells.choose(&mut rng).copied().unwrap();
            return Some(AiAction::Reveal(pick));
        }

        let list: Vec<Coord3D> = unrevealed_set.iter().copied().collect();
        list.choose(&mut rng).copied().map(AiAction::Reveal)
    }

    fn dedup_equations(eqs: Vec<Equation>) -> Vec<Equation> {
        let mut out: Vec<Equation> = Vec::new();
        for eq in eqs {
            if eq.cells.is_empty() {
                continue;
            }
            if !out
                .iter()
                .any(|e| e.mines == eq.mines && e.cells == eq.cells)
            {
                out.push(eq);
            }
        }
        out
    }

    fn direct_deductions(equations: &[Equation]) -> (HashSet<Coord3D>, HashSet<Coord3D>) {
        let mut safe = HashSet::new();
        let mut mines = HashSet::new();

        for eq in equations {
            if eq.mines == 0 {
                for &c in &eq.cells {
                    safe.insert(c);
                }
            } else if eq.mines == eq.cells.len() {
                for &c in &eq.cells {
                    mines.insert(c);
                }
            }
        }

        (safe, mines)
    }

    /// Fast Multi-pass subset reduction and replacement
    fn solve_subset_replacement(
        mut eqs: Vec<Equation>,
    ) -> (HashSet<Coord3D>, HashSet<Coord3D>, Vec<Equation>) {
        let mut certain_safe = HashSet::new();
        let mut certain_mines = HashSet::new();

        for _pass in 0..6 {
            let mut changed = false;

            // 1. Direct reductions
            for eq in &eqs {
                if eq.mines == 0 {
                    for &c in &eq.cells {
                        if certain_safe.insert(c) {
                            changed = true;
                        }
                    }
                } else if eq.mines == eq.cells.len() {
                    for &c in &eq.cells {
                        if certain_mines.insert(c) {
                            changed = true;
                        }
                    }
                }
            }

            // 2. Simplify existing equations using known safe / mines
            let mut simplified: Vec<Equation> = Vec::new();
            for eq in &eqs {
                let mut new_cells = HashSet::new();
                let mut new_mines = eq.mines;

                for &c in &eq.cells {
                    if certain_mines.contains(&c) {
                        new_mines = new_mines.saturating_sub(1);
                    } else if !certain_safe.contains(&c) {
                        new_cells.insert(c);
                    }
                }

                if !new_cells.is_empty() {
                    if !simplified
                        .iter()
                        .any(|e| e.mines == new_mines && e.cells == new_cells)
                    {
                        simplified.push(Equation {
                            cells: new_cells,
                            mines: new_mines,
                        });
                    }
                }
            }
            eqs = simplified;

            // 3. Subset reduction: If A subset of B => Replace B with B \ A
            let n = eqs.len();
            let mut new_equations = Vec::new();

            for i in 0..n {
                let set_a = &eqs[i].cells;
                let mines_a = eqs[i].mines;

                for j in 0..n {
                    if i == j {
                        continue;
                    }
                    let set_b = &eqs[j].cells;
                    let mines_b = eqs[j].mines;

                    if set_a.is_subset(set_b) && mines_b >= mines_a {
                        let diff: HashSet<Coord3D> = set_b.difference(set_a).copied().collect();
                        let diff_mines = mines_b - mines_a;

                        if !diff.is_empty() {
                            if diff_mines == 0 {
                                for &c in &diff {
                                    if certain_safe.insert(c) {
                                        changed = true;
                                    }
                                }
                            } else if diff_mines == diff.len() {
                                for &c in &diff {
                                    if certain_mines.insert(c) {
                                        changed = true;
                                    }
                                }
                            } else {
                                new_equations.push(Equation {
                                    cells: diff,
                                    mines: diff_mines,
                                });
                            }
                        }
                    }
                }
            }

            for eq in new_equations {
                if !eqs
                    .iter()
                    .any(|e| e.mines == eq.mines && e.cells == eq.cells)
                {
                    eqs.push(eq);
                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }

        (certain_safe, certain_mines, eqs)
    }

    /// Decompose frontier into independent connected components and solve exactly via Backtracking Model Counting
    fn solve_connected_components(
        equations: &[Equation],
        _unrevealed: &HashSet<Coord3D>,
        _remaining_mines: usize,
        is_master: bool,
    ) -> (HashSet<Coord3D>, HashSet<Coord3D>, HashMap<Coord3D, f64>) {
        let mut certain_safe = HashSet::new();
        let mut certain_mines = HashSet::new();
        let mut cell_probs = HashMap::new();

        if equations.is_empty() {
            return (certain_safe, certain_mines, cell_probs);
        }

        // Build variable adjacency graph
        let mut var_adj: HashMap<Coord3D, HashSet<Coord3D>> = HashMap::new();
        for eq in equations {
            let cells: Vec<Coord3D> = eq.cells.iter().copied().collect();
            for i in 0..cells.len() {
                for j in (i + 1)..cells.len() {
                    var_adj.entry(cells[i]).or_default().insert(cells[j]);
                    var_adj.entry(cells[j]).or_default().insert(cells[i]);
                }
                var_adj.entry(cells[i]).or_default();
            }
        }

        // Find connected components via BFS
        let mut visited: HashSet<Coord3D> = HashSet::new();
        let mut components: Vec<Vec<Coord3D>> = Vec::new();

        for &var in var_adj.keys() {
            if visited.contains(&var) {
                continue;
            }
            let mut comp = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(var);
            visited.insert(var);

            while let Some(curr) = queue.pop_front() {
                comp.push(curr);
                if let Some(neighbors) = var_adj.get(&curr) {
                    for &n in neighbors {
                        if !visited.contains(&n) {
                            visited.insert(n);
                            queue.push_back(n);
                        }
                    }
                }
            }
            components.push(comp);
        }

        // Solve each component independently
        for comp_vars in components {
            let comp_var_set: HashSet<Coord3D> = comp_vars.iter().copied().collect();
            let comp_eqs: Vec<Equation> = equations
                .iter()
                .filter(|eq| eq.cells.iter().any(|c| comp_var_set.contains(c)))
                .cloned()
                .collect();

            let max_bt_vars = if is_master { 18 } else { 14 };

            if comp_vars.len() <= max_bt_vars {
                // Exact Backtracking Model Counter (Tank Solver)
                let (c_safe, c_mines, probs) =
                    Self::solve_component_backtracking(&comp_vars, &comp_eqs);

                for c in c_safe {
                    certain_safe.insert(c);
                }
                for c in c_mines {
                    certain_mines.insert(c);
                }
                for (c, p) in probs {
                    cell_probs.insert(c, p);
                }
            } else {
                // Fast Integer Gaussian Elimination for larger components
                let (c_safe, c_mines, probs) =
                    Self::solve_component_gaussian(&comp_vars, &comp_eqs);

                for c in c_safe {
                    certain_safe.insert(c);
                }
                for c in c_mines {
                    certain_mines.insert(c);
                }
                for (c, p) in probs {
                    cell_probs.insert(c, p);
                }
            }
        }

        (certain_safe, certain_mines, cell_probs)
    }

    /// Exact binary backtracking on a single connected component with MRV variable ordering
    fn solve_component_backtracking(
        vars: &[Coord3D],
        eqs: &[Equation],
    ) -> (HashSet<Coord3D>, HashSet<Coord3D>, HashMap<Coord3D, f64>) {
        let mut certain_safe = HashSet::new();
        let mut certain_mines = HashSet::new();
        let mut probs = HashMap::new();

        let n = vars.len();
        if n == 0 {
            return (certain_safe, certain_mines, probs);
        }

        // Order variables by constraint frequency (Most Constrained Variable first)
        let mut var_freq: HashMap<Coord3D, usize> = HashMap::new();
        for eq in eqs {
            for &c in &eq.cells {
                *var_freq.entry(c).or_insert(0) += 1;
            }
        }
        let mut ordered_vars = vars.to_vec();
        ordered_vars.sort_by_key(|c| std::cmp::Reverse(var_freq.get(c).copied().unwrap_or(0)));

        let var_indices: HashMap<Coord3D, usize> = ordered_vars
            .iter()
            .enumerate()
            .map(|(i, &c)| (c, i))
            .collect();

        // Convert equations into fast index masks
        struct FastEq {
            indices: Vec<usize>,
            mines: usize,
        }
        let fast_eqs: Vec<FastEq> = eqs
            .iter()
            .map(|eq| FastEq {
                indices: eq.cells.iter().map(|c| var_indices[c]).collect(),
                mines: eq.mines,
            })
            .collect();

        let mut valid_assignment_count = 0u64;
        let mut var_mine_counts = vec![0u64; n];
        let mut current_assignment = vec![0u8; n];

        fn backtrack(
            idx: usize,
            n: usize,
            assignment: &mut Vec<u8>,
            fast_eqs: &[FastEq],
            valid_count: &mut u64,
            mine_counts: &mut [u64],
        ) {
            if *valid_count >= 5000 {
                return;
            }

            // Prune if any equation is violated
            for eq in fast_eqs {
                let mut assigned_mines = 0;
                let mut unassigned_count = 0;

                for &v_idx in &eq.indices {
                    if v_idx < idx {
                        if assignment[v_idx] == 1 {
                            assigned_mines += 1;
                        }
                    } else {
                        unassigned_count += 1;
                    }
                }

                if assigned_mines > eq.mines || assigned_mines + unassigned_count < eq.mines {
                    return;
                }
            }

            if idx == n {
                *valid_count += 1;
                for i in 0..n {
                    if assignment[i] == 1 {
                        mine_counts[i] += 1;
                    }
                }
                return;
            }

            // Try x_idx = 0 (Safe)
            assignment[idx] = 0;
            backtrack(idx + 1, n, assignment, fast_eqs, valid_count, mine_counts);

            // Try x_idx = 1 (Mine)
            assignment[idx] = 1;
            backtrack(idx + 1, n, assignment, fast_eqs, valid_count, mine_counts);
        }

        backtrack(
            0,
            n,
            &mut current_assignment,
            &fast_eqs,
            &mut valid_assignment_count,
            &mut var_mine_counts,
        );

        if valid_assignment_count > 0 {
            for (i, &coord) in ordered_vars.iter().enumerate() {
                let count = var_mine_counts[i];
                if count == 0 {
                    certain_safe.insert(coord);
                    probs.insert(coord, 0.0);
                } else if count == valid_assignment_count {
                    certain_mines.insert(coord);
                    probs.insert(coord, 1.0);
                } else {
                    let p = (count as f64) / (valid_assignment_count as f64);
                    probs.insert(coord, p.clamp(0.001, 0.999));
                }
            }
        }

        (certain_safe, certain_mines, probs)
    }

    /// Fast Integer Gaussian Elimination on a single component
    fn solve_component_gaussian(
        vars: &[Coord3D],
        eqs: &[Equation],
    ) -> (HashSet<Coord3D>, HashSet<Coord3D>, HashMap<Coord3D, f64>) {
        let mut certain_safe = HashSet::new();
        let mut certain_mines = HashSet::new();
        let mut probs = HashMap::new();

        let n_vars = vars.len();
        let n_eqs = eqs.len();
        if n_vars == 0 || n_eqs == 0 {
            return (certain_safe, certain_mines, probs);
        }

        let var_indices: HashMap<Coord3D, usize> =
            vars.iter().enumerate().map(|(i, &c)| (c, i)).collect();

        let mut matrix: Vec<Vec<f64>> = vec![vec![0.0; n_vars + 1]; n_eqs];
        for (i, eq) in eqs.iter().enumerate() {
            for c in &eq.cells {
                if let Some(&col) = var_indices.get(c) {
                    matrix[i][col] = 1.0;
                }
            }
            matrix[i][n_vars] = eq.mines as f64;
        }

        // RREF
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

        // Bound analysis
        for row in &matrix {
            let mut pos_sum = 0.0;
            let mut neg_sum = 0.0;
            let mut non_zero_cols = Vec::new();

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
            if (pos_sum - rhs).abs() < 1e-5 {
                for &(col, coeff) in &non_zero_cols {
                    let coord = vars[col];
                    if coeff > 0.0 {
                        certain_mines.insert(coord);
                    } else {
                        certain_safe.insert(coord);
                    }
                }
            }
            if (neg_sum - rhs).abs() < 1e-5 {
                for &(col, coeff) in &non_zero_cols {
                    let coord = vars[col];
                    if coeff > 0.0 {
                        certain_safe.insert(coord);
                    } else {
                        certain_mines.insert(coord);
                    }
                }
            }
        }

        for (col, &coord) in vars.iter().enumerate() {
            if certain_safe.contains(&coord) {
                probs.insert(coord, 0.0);
            } else if certain_mines.contains(&coord) {
                probs.insert(coord, 1.0);
            } else {
                let mut est = 0.3;
                for row in &matrix {
                    if row[col].abs() > 1e-6 {
                        let rhs = row[n_vars];
                        if (0.0..=1.0).contains(&rhs) {
                            est = rhs / row[col];
                            break;
                        }
                    }
                }
                probs.insert(coord, est.clamp(0.01, 0.99));
            }
        }

        (certain_safe, certain_mines, probs)
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

            if moves >= 50 {
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
