use crate::topology::{Coord3D, Dimensions};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Expert,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardConfig {
    pub difficulty: Difficulty,
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub mines: usize,
}

impl BoardConfig {
    #[inline]
    pub const fn dims(&self) -> Dimensions {
        Dimensions::new(self.width, self.height, self.depth)
    }

    pub fn easy() -> Self {
        Self {
            difficulty: Difficulty::Easy,
            width: 9,
            height: 9,
            depth: 3,
            mines: 25,
        }
    }

    pub fn medium() -> Self {
        Self {
            difficulty: Difficulty::Medium,
            width: 16,
            height: 16,
            depth: 4,
            mines: 160,
        }
    }

    pub fn expert() -> Self {
        Self {
            difficulty: Difficulty::Expert,
            width: 30,
            height: 16,
            depth: 6,
            mines: 580,
        }
    }

    pub fn custom(width: usize, height: usize, depth: usize, mines: usize) -> Result<Self, String> {
        if width < 3 || height < 3 || depth < 2 {
            return Err("Dimensions must be at least 3x3x2".to_string());
        }
        let total = width * height * depth;
        let max_density_mines = (total as f64 * 0.6).floor() as usize;
        let max_safe_mines = total.saturating_sub(27);

        let max_allowed = max_density_mines.min(max_safe_mines);
        if mines == 0 {
            return Err("Mine count must be greater than 0".to_string());
        }
        if mines > max_allowed {
            return Err(format!(
                "Mine count exceeds maximum allowable limit ({max_allowed})"
            ));
        }

        Ok(Self {
            difficulty: Difficulty::Custom,
            width,
            height,
            depth,
            mines,
        })
    }

    pub fn dimensions(&self) -> Dimensions {
        Dimensions::new(self.width, self.height, self.depth)
    }

    pub fn total_cells(&self) -> usize {
        self.width * self.height * self.depth
    }

    pub fn config_hash(&self) -> String {
        format!(
            "{}-{}-{}-{}",
            self.width, self.height, self.depth, self.mines
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub coord: Coord3D,
    pub is_mine: bool,
    pub adjacent_mines: u8,
    pub is_revealed: bool,
    pub is_flagged: bool,
    pub revealed_by: Option<String>,
    pub player_color: Option<String>,
}

impl Cell {
    pub fn new(coord: Coord3D) -> Self {
        Self {
            coord,
            is_mine: false,
            adjacent_mines: 0,
            is_revealed: false,
            is_flagged: false,
            revealed_by: None,
            player_color: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    Waiting,
    Playing,
    Won,
    Lost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevealedCellInfo {
    pub coord: Coord3D,
    pub adjacent_mines: u8,
    pub is_mine: bool,
    pub revealed_by: Option<String>,
    pub player_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RevealResult {
    FirstClickGenerated {
        revealed: Vec<RevealedCellInfo>,
    },
    Success {
        revealed: Vec<RevealedCellInfo>,
    },
    HitMine {
        hit_coord: Coord3D,
        all_mines: Vec<Coord3D>,
        revealed_by: Option<String>,
    },
    AlreadyRevealed,
    Flagged,
    NoOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub config: BoardConfig,
    pub dims: Dimensions,
    pub cells: Vec<Cell>,
    pub is_generated: bool,
    pub status: GameStatus,
    pub revealed_count: usize,
    pub flag_count: usize,
}

impl Board {
    pub fn new(config: BoardConfig) -> Self {
        let dims = config.dimensions();
        let total = dims.total_cells();
        let mut cells = Vec::with_capacity(total);

        for idx in 0..total {
            let coord = Coord3D::from_index(idx, dims.width, dims.height);
            cells.push(Cell::new(coord));
        }

        Self {
            config,
            dims,
            cells,
            is_generated: false,
            status: GameStatus::Waiting,
            revealed_count: 0,
            flag_count: 0,
        }
    }

    #[inline]
    pub fn get_cell(&self, coord: Coord3D) -> &Cell {
        let idx = coord.to_index(self.dims.width, self.dims.height);
        &self.cells[idx]
    }

    #[inline]
    pub fn get_cell_mut(&mut self, coord: Coord3D) -> &mut Cell {
        let idx = coord.to_index(self.dims.width, self.dims.height);
        &mut self.cells[idx]
    }

    /// Performs lazy generation on first click $(x_0, y_0, z_0)$ ensuring that the
    /// initial coordinate and all of its 26-neighbors (total <= 27 cells) are 100% mine-free.
    pub fn generate_mines_with_seed(&mut self, safe_center: Coord3D, seed: Option<u64>) {
        if self.is_generated {
            return;
        }

        // Collect safe zone: center + 26 neighbors (using Mobius topology)
        let mut safe_indices = HashSet::new();
        safe_indices.insert(safe_center.to_index(self.dims.width, self.dims.height));

        for neighbor in self.dims.get_neighbors(safe_center) {
            safe_indices.insert(neighbor.to_index(self.dims.width, self.dims.height));
        }

        let total = self.dims.total_cells();
        let mut available_indices: Vec<usize> = (0..total)
            .filter(|idx| !safe_indices.contains(idx))
            .collect();

        // Shuffle and pick M mines
        let mut rng = match seed {
            Some(s) => rand::rngs::SmallRng::seed_from_u64(s),
            None => rand::rngs::SmallRng::from_entropy(),
        };

        available_indices.shuffle(&mut rng);

        let mine_count = self.config.mines.min(available_indices.len());
        for &idx in available_indices.iter().take(mine_count) {
            self.cells[idx].is_mine = true;
        }

        // Calculate adjacent mine counts for all cells
        for idx in 0..total {
            let coord = Coord3D::from_index(idx, self.dims.width, self.dims.height);
            let neighbors = self.dims.get_neighbors(coord);
            let count = neighbors
                .iter()
                .filter(|&&n_coord| {
                    let n_idx = n_coord.to_index(self.dims.width, self.dims.height);
                    self.cells[n_idx].is_mine
                })
                .count() as u8;
            self.cells[idx].adjacent_mines = count;
        }

        self.is_generated = true;
        self.status = GameStatus::Playing;
    }

    /// Toggle flag state on an unrevealed cell
    pub fn toggle_flag(&mut self, coord: Coord3D) -> bool {
        if self.status == GameStatus::Lost || self.status == GameStatus::Won {
            return false;
        }
        if !self.dims.is_valid_coord(coord) {
            return false;
        }
        let cell = self.get_cell_mut(coord);
        if cell.is_revealed {
            return false;
        }
        cell.is_flagged = !cell.is_flagged;
        if cell.is_flagged {
            self.flag_count += 1;
        } else {
            self.flag_count = self.flag_count.saturating_sub(1);
        }
        true
    }

    /// Reveals a cell at coordinate `coord`, attributing the action to `player_id` with `player_color`.
    pub fn reveal(
        &mut self,
        coord: Coord3D,
        player_id: Option<String>,
        player_color: Option<String>,
    ) -> RevealResult {
        if self.status == GameStatus::Lost || self.status == GameStatus::Won {
            return RevealResult::NoOp;
        }
        if !self.dims.is_valid_coord(coord) {
            return RevealResult::NoOp;
        }

        let first_click = !self.is_generated;
        if first_click {
            self.generate_mines_with_seed(coord, None);
        }

        let cell = self.get_cell(coord);
        if cell.is_flagged {
            return RevealResult::Flagged;
        }
        if cell.is_revealed {
            return RevealResult::AlreadyRevealed;
        }

        // Check if hit mine
        if cell.is_mine {
            self.status = GameStatus::Lost;

            let all_mines: Vec<Coord3D> = self
                .cells
                .iter()
                .filter(|c| c.is_mine)
                .map(|c| c.coord)
                .collect();

            // Reveal all mines on game loss
            for c in self.cells.iter_mut() {
                if c.is_mine {
                    c.is_revealed = true;
                    if c.coord == coord {
                        c.revealed_by = player_id.clone();
                        c.player_color = player_color.clone();
                    }
                }
            }

            return RevealResult::HitMine {
                hit_coord: coord,
                all_mines,
                revealed_by: player_id,
            };
        }

        // Execute 3D Flood Fill BFS across Mobius and Z boundaries
        let mut revealed_list = Vec::new();
        let mut queue = VecDeque::new();

        let initial_idx = coord.to_index(self.dims.width, self.dims.height);
        queue.push_back(initial_idx);

        let cell_mut = &mut self.cells[initial_idx];
        cell_mut.is_revealed = true;
        cell_mut.revealed_by = player_id.clone();
        cell_mut.player_color = player_color.clone();
        self.revealed_count += 1;

        revealed_list.push(RevealedCellInfo {
            coord,
            adjacent_mines: cell_mut.adjacent_mines,
            is_mine: false,
            revealed_by: player_id.clone(),
            player_color: player_color.clone(),
        });

        while let Some(current_idx) = queue.pop_front() {
            let current_cell = &self.cells[current_idx];
            if current_cell.adjacent_mines != 0 {
                // Numbered cells do not expand further
                continue;
            }

            let current_coord = current_cell.coord;
            let neighbors = self.dims.get_neighbors(current_coord);

            for neighbor_coord in neighbors {
                let n_idx = neighbor_coord.to_index(self.dims.width, self.dims.height);
                let neighbor_cell = &mut self.cells[n_idx];

                if !neighbor_cell.is_revealed && !neighbor_cell.is_flagged && !neighbor_cell.is_mine
                {
                    neighbor_cell.is_revealed = true;
                    neighbor_cell.revealed_by = player_id.clone();
                    neighbor_cell.player_color = player_color.clone();
                    self.revealed_count += 1;

                    revealed_list.push(RevealedCellInfo {
                        coord: neighbor_coord,
                        adjacent_mines: neighbor_cell.adjacent_mines,
                        is_mine: false,
                        revealed_by: player_id.clone(),
                        player_color: player_color.clone(),
                    });

                    if neighbor_cell.adjacent_mines == 0 {
                        queue.push_back(n_idx);
                    }
                }
            }
        }

        // Check if all non-mine cells are revealed
        let non_mine_total = self.dims.total_cells() - self.config.mines;
        if self.revealed_count >= non_mine_total {
            self.status = GameStatus::Won;
        }

        if first_click {
            RevealResult::FirstClickGenerated {
                revealed: revealed_list,
            }
        } else {
            RevealResult::Success {
                revealed: revealed_list,
            }
        }
    }

    /// Chord action: If a revealed cell's adjacent flags == adjacent mines count,
    /// reveal all adjacent unrevealed & unflagged neighbors.
    pub fn chord(
        &mut self,
        coord: Coord3D,
        player_id: Option<String>,
        player_color: Option<String>,
    ) -> RevealResult {
        if self.status == GameStatus::Lost || self.status == GameStatus::Won {
            return RevealResult::NoOp;
        }
        if !self.dims.is_valid_coord(coord) {
            return RevealResult::NoOp;
        }

        let cell = self.get_cell(coord);
        if !cell.is_revealed || cell.adjacent_mines == 0 {
            return RevealResult::NoOp;
        }

        let neighbors = self.dims.get_neighbors(coord);
        let flag_count = neighbors
            .iter()
            .filter(|&&nc| self.get_cell(nc).is_flagged)
            .count() as u8;

        if flag_count != cell.adjacent_mines {
            return RevealResult::NoOp;
        }

        let mut combined_revealed = Vec::new();

        for nc in neighbors {
            let neighbor_cell = self.get_cell(nc);
            if !neighbor_cell.is_revealed && !neighbor_cell.is_flagged {
                match self.reveal(nc, player_id.clone(), player_color.clone()) {
                    RevealResult::HitMine {
                        hit_coord,
                        all_mines,
                        revealed_by,
                    } => {
                        return RevealResult::HitMine {
                            hit_coord,
                            all_mines,
                            revealed_by,
                        };
                    }
                    RevealResult::Success { mut revealed }
                    | RevealResult::FirstClickGenerated { mut revealed } => {
                        combined_revealed.append(&mut revealed);
                    }
                    _ => {}
                }
            }
        }

        RevealResult::Success {
            revealed: combined_revealed,
        }
    }

    pub fn is_won(&self) -> bool {
        self.status == GameStatus::Won
    }

    pub fn is_lost(&self) -> bool {
        self.status == GameStatus::Lost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_click_safety_guarantee() {
        let config = BoardConfig::easy();
        let mut board = Board::new(config);
        let first_click_coord = Coord3D::new(4, 4, 1);

        let result = board.reveal(first_click_coord, Some("test_user".into()), None);

        // Verify the board is generated and first click cell is revealed
        assert!(board.is_generated);
        assert_eq!(board.get_cell(first_click_coord).adjacent_mines, 0);
        assert!(!board.get_cell(first_click_coord).is_mine);

        // Verify all 26 neighbors of the first click are 100% mine-free
        for neighbor in board.dims.get_neighbors(first_click_coord) {
            assert!(
                !board.get_cell(neighbor).is_mine,
                "Neighbor {:?} was incorrectly placed with a mine!",
                neighbor
            );
        }

        match result {
            RevealResult::FirstClickGenerated { revealed } => {
                // Must have revealed at least 27 cells
                assert!(
                    revealed.len() >= 27,
                    "Expected >= 27 revealed cells, got {}",
                    revealed.len()
                );
            }
            _ => panic!("Expected FirstClickGenerated result"),
        }
    }

    #[test]
    fn test_custom_profile_density_validation() {
        // Valid custom
        assert!(BoardConfig::custom(10, 10, 3, 50).is_ok());

        // Invalid: > 60% density
        assert!(BoardConfig::custom(10, 10, 3, 190).is_err());

        // Invalid: leaves less than 27 safe spots
        assert!(BoardConfig::custom(3, 3, 3, 1).is_err()); // 27 total - 27 safe = 0 max
    }
}
