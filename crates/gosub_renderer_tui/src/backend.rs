use std::any::Any;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use gosub_interface::font_system::FontSystem;
use gosub_interface::render::backend::{
    ErasedSurface, ExternalHandle, PresentMode, RasterStrategy, RenderBackend, RgbaImage, SurfaceSize,
};
use gosub_interface::render::render_context::RenderContext;
use gosub_render_pipeline::rasterizer::erase_rasterizer;
use parking_lot::Mutex;

use crate::cell::CellCanvas;
use crate::rasterizer::TuiRasterizer;

/// A render backend that produces character cells.
///
/// Structurally this is the null backend plus a rasterizer: the display-list methods stay inert
/// (`render` does nothing, `external_handle` returns `NullHandle`) because the cell grid leaves
/// through [`TuiBackend::canvas`], not through the compositor.
pub struct TuiBackend {
    canvas: Arc<Mutex<CellCanvas>>,
}

impl TuiBackend {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            canvas: Arc::new(Mutex::new(CellCanvas::new())),
        }
    }

    /// The grid the rasterizer draws into. The host clones this and reads it to paint the terminal.
    pub fn canvas(&self) -> Arc<Mutex<CellCanvas>> {
        self.canvas.clone()
    }
}

impl RenderBackend for TuiBackend {
    fn name(&self) -> &'static str {
        "TuiBackend"
    }

    fn create_surface(&self, size: SurfaceSize, _present: PresentMode) -> Result<Box<dyn ErasedSurface + Send>> {
        Ok(Box::new(TuiSurface::new(size)))
    }

    fn render(&self, _ctx: &mut dyn RenderContext, surface: &mut dyn ErasedSurface) -> Result<()> {
        // Never called in practice: `raster_strategy` is not `None` and we don't render to a GPU
        // texture, so the tab worker takes the TileCache path and skips the display list.
        let s = surface
            .as_any_mut()
            .downcast_mut::<TuiSurface>()
            .ok_or_else(|| anyhow!("TuiBackend used with non-Tui surface"))?;

        s.frame_id = s.frame_id.wrapping_add(1);
        Ok(())
    }

    fn snapshot(&self, _surface: &mut dyn ErasedSurface, _max_dim: u32) -> Result<RgbaImage> {
        Err(anyhow!(
            "TuiBackend renders character cells, not pixels; snapshot is unsupported"
        ))
    }

    fn external_handle(&self, surface: &mut dyn ErasedSurface) -> Result<ExternalHandle> {
        let s = surface
            .as_any_mut()
            .downcast_mut::<TuiSurface>()
            .ok_or_else(|| anyhow!("TuiBackend used with non-Tui surface"))?;

        Ok(ExternalHandle::NullHandle {
            width: s.size.width,
            height: s.size.height,
            frame_id: s.frame_id,
        })
    }

    fn create_rasterizer(&self, font_system: Arc<Mutex<dyn FontSystem>>) -> Box<dyn Any + Send + Sync> {
        erase_rasterizer(Box::new(TuiRasterizer::new(self.canvas.clone(), font_system)))
    }

    fn raster_strategy(&self) -> RasterStrategy {
        // Sequential rather than ParallelCached: the content-hash pixel cache would let the engine
        // skip `rasterize` for unchanged tiles, and since our output leaves through the canvas
        // side-channel rather than the texture store, a skipped tile is a tile that never gets
        // drawn. Cheap regardless — an 80×24 grid is not worth caching.
        RasterStrategy::Sequential
    }
}

/// Surface for [`TuiBackend`]. Holds the size the engine asked for and a frame counter; there are
/// no pixels behind it.
pub struct TuiSurface {
    pub size: SurfaceSize,
    frame_id: u64,
}

impl TuiSurface {
    pub fn new(size: SurfaceSize) -> Self {
        Self { size, frame_id: 0 }
    }
}

impl ErasedSurface for TuiSurface {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn size(&self) -> SurfaceSize {
        self.size
    }
}
