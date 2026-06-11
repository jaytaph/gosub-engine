//! Browser binary: GTK4 GLArea + Skia (CPU rasterize, GPU composite).
//!
//! Usage: cargo run -p example-browser --no-default-features --features gtk4-skia-gpu --bin gtk4-skia-gpu -- https://example.com
//!
//! Build slim (skia-only engine) — building together with cairo bins makes the
//! engine take the cairo rasterizer path.

use example_browser::common::{logging, rt};

fn main() {
    logging::init();
    rt::init("gosub-gtk4-skia-gpu-rt");

    example_browser::shell::gtk4::skia_gpu::run();
}
