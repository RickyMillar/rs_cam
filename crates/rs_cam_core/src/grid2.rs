//! Minimal generic row-major 2D grid — the shared "given a `(row, col)` or a
//! flat index, read/write bounds-safely" primitive.
//!
//! Convention: **row-major**, `index = r * nx + c`, where `r` is the row
//! (varies with Y) and `c` is the column (varies with X). This mirrors every
//! grid in the crate today — `rest_field`'s `RestGrid`, `SurfaceHeightmap`,
//! `SlopeMap`, `DropCutterGrid`, and `adaptive::material_grid`'s
//! `MaterialGrid` all use the same `row*cols+col` layout (grid census,
//! `planning/finishing_stack_review_2026-07.md` §R1.9).
//!
//! `rest_field.rs` is the first adopter (tracker P1.6): its internal working
//! buffers (rest depth, pencil Z, contact mask, valley mask, component ids,
//! distance transform, skeleton) are `Grid2<T>` instead of parallel `Vec<T>`
//! plus hand-rolled `r*nx+c` math repeated at every call site. `SurfaceHeightmap`,
//! `SlopeMap`, `DropCutterGrid`, and `MaterialGrid` are intended future
//! adopters — deliberately NOT migrated here, since each has its own cosmetic
//! divergence (naming, anisotropic cell steps, empty-cell policy) that
//! deserves its own look rather than a forced fit.
//!
//! All accessors are bounds-checked through `Vec::get`/`get_mut` — there is no
//! raw slice indexing anywhere in this module, so no `#[allow(clippy::indexing_slicing)]`
//! is needed at all.

use std::error::Error;
use std::fmt;

/// Flat row-major index for `(r, c)` in an `nx`-wide grid. Pure arithmetic —
/// call sites that already have a [`Grid2`] instance should prefer
/// [`Grid2::index_of`] (bounds-checked); this free function exists for the
/// rarer case of computing an index before the grid itself has been built
/// (e.g. an initial per-cell sampling pass).
#[inline]
pub fn row_major_index(r: usize, c: usize, nx: usize) -> usize {
    r * nx + c
}

/// Inverse of [`row_major_index`]: row/column for a flat index in an
/// `nx`-wide grid.
#[inline]
pub fn row_major_rc(i: usize, nx: usize) -> (usize, usize) {
    (i / nx, i % nx)
}

/// Error returned by [`Grid2::from_vec`] when `data.len() != nx * ny`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridShapeError {
    pub nx: usize,
    pub ny: usize,
    pub data_len: usize,
}

impl fmt::Display for GridShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Grid2::from_vec: {}x{} grid needs {} elements, got {}",
            self.nx,
            self.ny,
            self.nx * self.ny,
            self.data_len
        )
    }
}

impl Error for GridShapeError {}

/// A dense row-major 2D grid of `T`. See the module doc for the indexing
/// convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid2<T> {
    nx: usize,
    ny: usize,
    data: Vec<T>,
}

impl<T: Clone> Grid2<T> {
    /// A new `nx`×`ny` grid with every cell set to `fill`.
    pub fn new_fill(nx: usize, ny: usize, fill: T) -> Self {
        Self {
            nx,
            ny,
            data: vec![fill; nx * ny],
        }
    }
}

impl<T> Grid2<T> {
    /// Wrap an existing flat row-major buffer. `Err` when
    /// `data.len() != nx * ny`.
    pub fn from_vec(nx: usize, ny: usize, data: Vec<T>) -> Result<Self, GridShapeError> {
        if data.len() == nx * ny {
            Ok(Self { nx, ny, data })
        } else {
            Err(GridShapeError {
                nx,
                ny,
                data_len: data.len(),
            })
        }
    }

    pub fn nx(&self) -> usize {
        self.nx
    }

    pub fn ny(&self) -> usize {
        self.ny
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Flat index for `(r, c)`, or `None` if either is out of bounds.
    pub fn index_of(&self, r: usize, c: usize) -> Option<usize> {
        if r < self.ny && c < self.nx {
            Some(row_major_index(r, c, self.nx))
        } else {
            None
        }
    }

    /// Row/column for a flat index, or `None` if out of bounds.
    pub fn rc_of(&self, i: usize) -> Option<(usize, usize)> {
        if i < self.len() {
            Some(row_major_rc(i, self.nx))
        } else {
            None
        }
    }

    /// Flat index for a *signed* `(r, c)`, or `None` if negative or out of
    /// bounds. The one-call replacement for "is this 8-neighbourhood offset
    /// still on the grid, and if so what's its flat index" — the pattern
    /// every NB8 walk needs.
    pub fn index_of_signed(&self, r: isize, c: isize) -> Option<usize> {
        if r < 0 || c < 0 {
            return None;
        }
        self.index_of(r as usize, c as usize)
    }

    pub fn get(&self, r: usize, c: usize) -> Option<&T> {
        let i = self.index_of(r, c)?;
        self.data.get(i)
    }

    pub fn get_mut(&mut self, r: usize, c: usize) -> Option<&mut T> {
        let i = self.index_of(r, c)?;
        self.data.get_mut(i)
    }

    /// Set `(r, c)`. Returns `false` (no-op) if out of bounds.
    pub fn set(&mut self, r: usize, c: usize, val: T) -> bool {
        match self.get_mut(r, c) {
            Some(slot) => {
                *slot = val;
                true
            }
            None => false,
        }
    }

    pub fn at_index(&self, i: usize) -> Option<&T> {
        self.data.get(i)
    }

    pub fn at_index_mut(&mut self, i: usize) -> Option<&mut T> {
        self.data.get_mut(i)
    }

    /// Set a flat index. Returns `false` (no-op) if out of bounds.
    pub fn set_index(&mut self, i: usize, val: T) -> bool {
        match self.data.get_mut(i) {
            Some(slot) => {
                *slot = val;
                true
            }
            None => false,
        }
    }

    /// Iterate every cell as `(r, c, &value)` in row-major order.
    pub fn iter_rc(&self) -> impl Iterator<Item = (usize, usize, &T)> {
        let nx = self.nx;
        self.data
            .iter()
            .enumerate()
            .map(move |(i, v)| (i / nx, i % nx, v))
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn into_vec(self) -> Vec<T> {
        self.data
    }
}

impl<T: Copy> Grid2<T> {
    /// Read `(r, c)`, or `default` if out of bounds.
    pub fn get_or(&self, r: usize, c: usize, default: T) -> T {
        self.get(r, c).copied().unwrap_or(default)
    }

    /// Read a flat index, or `default` if out of bounds.
    pub fn at_index_or(&self, i: usize, default: T) -> T {
        self.data.get(i).copied().unwrap_or(default)
    }

    /// Signed-coordinate read used by 8-neighbourhood walks: a negative or
    /// out-of-grid `(r, c)` reads as `default` rather than wrapping/panicking.
    pub fn get_signed_or(&self, r: isize, c: isize, default: T) -> T {
        if r < 0 || c < 0 {
            return default;
        }
        self.get_or(r as usize, c as usize, default)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn new_fill_has_every_cell_set() {
        let g = Grid2::new_fill(3, 2, 7i32);
        assert_eq!(g.nx(), 3);
        assert_eq!(g.ny(), 2);
        assert_eq!(g.len(), 6);
        assert!(!g.is_empty());
        for r in 0..2 {
            for c in 0..3 {
                assert_eq!(g.get(r, c), Some(&7));
            }
        }
    }

    #[test]
    fn from_vec_ok_and_mismatch() {
        let ok = Grid2::from_vec(2, 2, vec![1, 2, 3, 4]);
        assert!(ok.is_ok());
        let bad = Grid2::from_vec(2, 2, vec![1, 2, 3]);
        match bad {
            Err(e) => {
                assert_eq!(e.nx, 2);
                assert_eq!(e.ny, 2);
                assert_eq!(e.data_len, 3);
            }
            Ok(_) => panic!("expected shape mismatch to be rejected"),
        }
    }

    #[test]
    fn index_of_and_rc_of_round_trip() {
        let g = Grid2::new_fill(4, 3, 0u8);
        for r in 0..3 {
            for c in 0..4 {
                let i = g.index_of(r, c).unwrap();
                assert_eq!(g.rc_of(i), Some((r, c)));
            }
        }
        assert_eq!(g.index_of(3, 0), None, "row out of bounds");
        assert_eq!(g.index_of(0, 4), None, "col out of bounds");
        assert_eq!(g.rc_of(12), None, "flat index out of bounds (4*3=12)");
    }

    #[test]
    fn index_of_signed_rejects_negative_and_out_of_bounds() {
        let g = Grid2::new_fill(4, 3, 0u8);
        assert_eq!(g.index_of_signed(1, 2), g.index_of(1, 2));
        assert_eq!(g.index_of_signed(-1, 0), None);
        assert_eq!(g.index_of_signed(0, -1), None);
        assert_eq!(g.index_of_signed(3, 0), None, "row out of bounds");
        assert_eq!(g.index_of_signed(0, 4), None, "col out of bounds");
    }

    #[test]
    fn get_set_mutate_round_trip() {
        let mut g = Grid2::new_fill(3, 3, 0.0f64);
        assert!(g.set(1, 2, 5.0));
        assert_eq!(g.get(1, 2), Some(&5.0));
        assert!(
            !g.set(10, 10, 9.0),
            "out-of-bounds set is a no-op, not a panic"
        );

        if let Some(slot) = g.get_mut(0, 0) {
            *slot = 42.0;
        }
        assert_eq!(g.get(0, 0), Some(&42.0));
    }

    #[test]
    fn at_index_set_index_round_trip() {
        let mut g = Grid2::new_fill(2, 2, false);
        assert!(g.set_index(3, true));
        assert_eq!(g.at_index(3), Some(&true));
        assert!(
            !g.set_index(4, true),
            "index 4 is out of bounds for a 2x2 grid"
        );
        assert_eq!(g.at_index(4), None);
    }

    #[test]
    fn iter_rc_visits_every_cell_in_row_major_order() {
        let g = Grid2::from_vec(3, 2, vec![0, 1, 2, 3, 4, 5]).unwrap();
        let visited: Vec<(usize, usize, i32)> = g.iter_rc().map(|(r, c, &v)| (r, c, v)).collect();
        assert_eq!(
            visited,
            vec![
                (0, 0, 0),
                (0, 1, 1),
                (0, 2, 2),
                (1, 0, 3),
                (1, 1, 4),
                (1, 2, 5),
            ]
        );
    }

    #[test]
    fn get_or_and_at_index_or_default_out_of_bounds() {
        let g = Grid2::new_fill(2, 2, 1i32);
        assert_eq!(g.get_or(0, 0, -1), 1);
        assert_eq!(g.get_or(5, 5, -1), -1);
        assert_eq!(g.at_index_or(0, -1), 1);
        assert_eq!(g.at_index_or(99, -1), -1);
    }

    #[test]
    fn get_signed_or_rejects_negative_coordinates() {
        let g = Grid2::new_fill(2, 2, 3i32);
        assert_eq!(g.get_signed_or(0, 0, -1), 3);
        assert_eq!(g.get_signed_or(-1, 0, -1), -1);
        assert_eq!(g.get_signed_or(0, -1, -1), -1);
        assert_eq!(g.get_signed_or(5, 5, -1), -1);
    }

    #[test]
    fn as_slice_and_into_vec() {
        let g = Grid2::from_vec(2, 1, vec![10, 20]).unwrap();
        assert_eq!(g.as_slice(), &[10, 20]);
        assert_eq!(g.into_vec(), vec![10, 20]);
    }

    #[test]
    fn row_major_helpers_are_inverses() {
        let nx = 7;
        for i in 0..21usize {
            let (r, c) = row_major_rc(i, nx);
            assert_eq!(row_major_index(r, c, nx), i);
        }
    }
}
