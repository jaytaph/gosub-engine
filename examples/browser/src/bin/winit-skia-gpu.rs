//! Browser binary: winit + Skia GPU (OpenGL/Ganesh) compositing.
//!
//! Usage: cargo run -p example-browser --no-default-features --features winit-skia-gpu --bin winit-skia-gpu -- https://example.com
//!
//! Build slim (skia-only engine) — building together with cairo bins makes the
//! engine take the cairo rasterizer path.

use example_browser::common::{logging, nav, rt};

fn main() {
    logging::init();
    rt::init("gosub-winit-skia-gpu-rt");

    let initial_url = nav::normalize_url(&nav::initial_url_from_args("https://example.com"));
    example_browser::shell::winit::skia_gpu::run(initial_url);
}
