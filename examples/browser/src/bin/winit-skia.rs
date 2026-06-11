//! Browser binary: winit toolkit + Skia (CPU) rasterizer + softbuffer presentation.
//!
//! Usage: cargo run -p example-browser --bin winit-skia -- https://example.com

use example_browser::common::{logging, nav, rt};

fn main() {
    logging::init();
    rt::init("gosub-winit-skia-rt");

    let initial_url = nav::normalize_url(&nav::initial_url_from_args("https://example.com"));
    example_browser::shell::winit::skia::run(initial_url);
}
