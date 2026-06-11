//! Browser binary: egui toolkit + Vello (GPU) rasterizer.
//!
//! Usage: cargo run -p example-browser --no-default-features --features egui-vello --bin egui-vello -- https://example.com
//!
//! Build slim (vello-only engine) — building together with cairo/skia bins makes
//! the engine take the cairo rasterizer path and this window stays blank.

use example_browser::common::{logging, nav, rt};

fn main() -> Result<(), eframe::Error> {
    logging::init();
    rt::init("gosub-egui-vello-rt");

    let initial_url = nav::normalize_url(&nav::initial_url_from_args("https://example.com"));
    example_browser::shell::egui::vello::run(initial_url)
}
