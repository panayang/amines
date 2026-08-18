use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Coord3D {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl Coord3D {
    #[inline]
    pub const fn new(x: usize, y: usize, z: usize) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn to_index(&self, width: usize, height: usize) -> usize {
        self.z * (width * height) + self.y * width + self.x
    }

    #[inline]
    pub fn neighbors_26(&self, dims: Dimensions) -> Vec<Coord3D> {
        dims.get_neighbors(*self)
    }

    #[inline]
    pub fn from_index(index: usize, width: usize, height: usize) -> Self {
        let area = width * height;
        let z = index / area;
        let rem = index % area;
        let y = rem / width;
        let x = rem % width;
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
}

impl Dimensions {
    #[inline]
    pub const fn new(width: usize, height: usize, depth: usize) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }

    #[inline]
    pub fn total_cells(&self) -> usize {
        self.width * self.height * self.depth
    }

    #[inline]
    pub fn is_valid_coord(&self, coord: Coord3D) -> bool {
        coord.x < self.width && coord.y < self.height && coord.z < self.depth
    }

    /// Map a tentative 3D coordinate (with possible X wrapping and Y/Z inversion)
    /// based on the 3D Möbius Strip Topology rules:
    /// - Height (Y) and Depth (Z) are bounded (no cross-boundary wrapping).
    /// - Crossing X = width maps to X' = 0 with Y' = (H - 1) - Y and Z' = (D - 1) - Z.
    /// - Crossing X = -1 maps to X' = width - 1 with Y' = (H - 1) - Y and Z' = (D - 1) - Z.
    ///
    /// Returns `Some(Coord3D)` if valid and in-bounds, or `None` if truncated on Y or Z.
    pub fn map_neighbor(&self, base: Coord3D, dx: i32, dy: i32, dz: i32) -> Option<Coord3D> {
        let target_x = base.x as i32 + dx;
        let tentative_y = base.y as i32 + dy;
        let tentative_z = base.z as i32 + dz;

        let (final_x, final_y, final_z) = if target_x >= self.width as i32 {
            // Crossing right boundary: X' = 0, double flip Y and Z
            let mapped_x = 0;
            let mapped_y = (self.height as i32 - 1) - tentative_y;
            let mapped_z = (self.depth as i32 - 1) - tentative_z;
            (mapped_x, mapped_y, mapped_z)
        } else if target_x < 0 {
            // Crossing left boundary: X' = W - 1, double flip Y and Z
            let mapped_x = self.width as i32 - 1;
            let mapped_y = (self.height as i32 - 1) - tentative_y;
            let mapped_z = (self.depth as i32 - 1) - tentative_z;
            (mapped_x, mapped_y, mapped_z)
        } else {
            // Within X bounds: standard linear coordinate
            (target_x, tentative_y, tentative_z)
        };

        // Check if Y and Z remain within valid bounded dimensions
        if final_y >= 0
            && (final_y as usize) < self.height
            && final_z >= 0
            && (final_z as usize) < self.depth
        {
            Some(Coord3D {
                x: final_x as usize,
                y: final_y as usize,
                z: final_z as usize,
            })
        } else {
            None
        }
    }

    /// Computes the 26-neighborhood for a given coordinate under the 3D Möbius topology.
    /// Deduplicates results (important if dimensions are very small).
    pub fn get_neighbors(&self, coord: Coord3D) -> Vec<Coord3D> {
        let mut neighbors = Vec::with_capacity(26);

        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }

                    if let Some(mapped) = self.map_neighbor(coord, dx, dy, dz) {
                        if !neighbors.contains(&mapped) {
                            neighbors.push(mapped);
                        }
                    }
                }
            }
        }

        neighbors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobius_boundary_right_crossing() {
        let dims = Dimensions::new(9, 9, 3);
        // Base at right edge: x = 8, y = 2, z = 0
        let base = Coord3D::new(8, 2, 0);
        // Step dx = +1, dy = 0, dz = 0 -> maps to x' = 0, y' = 8 - 2 = 6, z' = 2 - 0 = 2
        let neighbor = dims.map_neighbor(base, 1, 0, 0);
        assert_eq!(neighbor, Some(Coord3D::new(0, 6, 2)));
    }

    #[test]
    fn test_mobius_boundary_left_crossing() {
        let dims = Dimensions::new(9, 9, 3);
        // Base at left edge: x = 0, y = 7, z = 2
        let base = Coord3D::new(0, 7, 2);
        // Step dx = -1, dy = 0, dz = 0 -> maps to x' = 8, y' = 8 - 7 = 1, z' = 2 - 2 = 0
        let neighbor = dims.map_neighbor(base, -1, 0, 0);
        assert_eq!(neighbor, Some(Coord3D::new(8, 1, 0)));
    }

    #[test]
    fn test_vertical_truncation() {
        let dims = Dimensions::new(9, 9, 3);
        // Base at bottom edge: y = 8. Stepping down dy = +1 should be truncated.
        let base = Coord3D::new(4, 8, 1);
        let neighbor = dims.map_neighbor(base, 0, 1, 0);
        assert_eq!(neighbor, None);
    }

    #[test]
    fn test_depth_truncation() {
        let dims = Dimensions::new(9, 9, 3);
        // Base at deepest layer: z = 2. Stepping dz = +1 should be truncated.
        let base = Coord3D::new(4, 4, 2);
        let neighbor = dims.map_neighbor(base, 0, 0, 1);
        assert_eq!(neighbor, None);
    }

    #[test]
    fn test_corner_mobius_cross_diagonal() {
        let dims = Dimensions::new(9, 9, 3);
        // Base at x = 8, y = 0, z = 0
        // Step dx = +1, dy = 1, dz = 1
        // tentative: x' = 9 -> wraps to x'' = 0
        // tentative y' = 0 + 1 = 1 -> inverted: (9 - 1) - 1 = 7
        // tentative z' = 0 + 1 = 1 -> inverted: (3 - 1) - 1 = 1
        let base = Coord3D::new(8, 0, 0);
        let neighbor = dims.map_neighbor(base, 1, 1, 1);
        assert_eq!(neighbor, Some(Coord3D::new(0, 7, 1)));
    }

    #[test]
    fn test_coord_indexing() {
        let dims = Dimensions::new(16, 16, 4);
        for z in 0..dims.depth {
            for y in 0..dims.height {
                for x in 0..dims.width {
                    let coord = Coord3D::new(x, y, z);
                    let idx = coord.to_index(dims.width, dims.height);
                    let restored = Coord3D::from_index(idx, dims.width, dims.height);
                    assert_eq!(coord, restored);
                }
            }
        }
    }
}
