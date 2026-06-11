//! Browser binary: winit toolkit + Cairo (CPU) rasterizer + softbuffer presentation.
//!
//! Usage: cargo run -p example-browser --bin winit-cairo -- https://example.com

use example_browser::common::{logging, nav, rt};

fn main() {
    logging::init();
    rt::init("gosub-winit-cairo-rt");

    let initial_url = nav::normalize_url(&nav::initial_url_from_args("https://example.com"));
    example_browser::shell::winit::cairo::run(initial_url);
}
