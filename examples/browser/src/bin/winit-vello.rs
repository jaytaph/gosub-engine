//! Browser binary: winit toolkit + Vello (GPU) rasterizer.
//!
//! Usage: cargo run -p example-browser --no-default-features --features winit-vello --bin winit-vello -- https://example.com
//!
//! Build slim (vello-only engine) — building together with cairo/skia bins makes
//! the engine take the cairo rasterizer path and this window stays blank.

use example_browser::common::{logging, nav, rt};

fn main() {
    logging::init();
    rt::init("gosub-winit-vello-rt");

    let initial_url = nav::normalize_url(&nav::initial_url_from_args("https://example.com"));
    example_browser::shell::winit::vello::run(initial_url);
}
