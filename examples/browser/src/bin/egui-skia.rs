//! Browser binary: egui toolkit + Skia (CPU) rasterizer.
//!
//! No GTK dependency — Skia has its own font system.
//!
//! Usage: cargo run -p example-browser --bin egui-skia -- https://example.com

use example_browser::common::engine::SetupOptions;
use example_browser::common::{logging, nav, rt};
use example_browser::shell::egui::cpu::{run, EguiCpuOptions};
use std::sync::Arc;

fn main() -> Result<(), eframe::Error> {
    logging::init();
    rt::init("gosub-egui-skia-rt");

    let initial_url = nav::normalize_url(&nav::initial_url_from_args("https://example.com"));
    run(
        initial_url,
        EguiCpuOptions {
            window_title: "Gosub Browser — egui + Skia",
            setup: SetupOptions {
                zone_uuid: uuid::uuid!("f1234567-abcd-4000-8000-000000000009"),
                tab_title: "Gosub",
                local_store_path: ":memory:",
                initial_viewport: None,
            },
            make_backend: || Arc::new(gosub_render_pipeline::render::backends::skia::SkiaBackend::new()),
            init_gtk: false,
            dpr_sink: None,
        },
    )
}
