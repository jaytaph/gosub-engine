//! Text-mode render backend — a proof-of-concept that renders a page into a grid of character
//! cells instead of pixels.
//!
//! The engine's pixel backends (Cairo, Skia, Vello) hook in at stage 6 and produce tile pixels
//! that the host composites. This backend hooks in at the same place but writes **cells**, and
//! deliberately skips the tile/composite machinery entirely: `rasterize` returns `None` (so every
//! tile is `Empty` and the `TileCache` handle is empty), and the cell grid is handed to the host
//! out-of-band through a shared [`CellCanvas`]. That keeps the whole experiment inside this crate
//! — no new `PixelFormat`, no new `ExternalHandle` variant, no changes to `gosub_interface`.
//!
//! The load-bearing piece is [`CellFontSystem`], not the rasterizer. Layout measures text through
//! the configured `FontSystem`, so reporting cell-quantized metrics there makes Taffy produce a
//! box tree that is already aligned to the character grid. Without it, a page laid out with
//! proportional metrics and quantized afterwards collides with itself.
//!
//! Skeleton scope: text and solid rectangle backgrounds. Borders, border-radius, `Svg`, gradient
//! and image brushes are ignored, as are `letter-spacing`, `text-align`, and bold/italic. A cell
//! holds one opaque colour, so fills below `ALPHA_CUTOFF` are skipped rather than blended.

pub mod backend;
pub mod cell;
pub mod font;
pub mod rasterizer;

pub use backend::{TuiBackend, TuiSurface};
pub use cell::{Background, Cell, CellCanvas};
pub use font::{CellFontSystem, CELL_H, CELL_W};
pub use rasterizer::TuiRasterizer;
