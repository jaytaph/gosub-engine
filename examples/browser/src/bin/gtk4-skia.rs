//! Browser binary: GTK4 toolkit + Skia (CPU) rasterizer.
//!
//! GTK4 is used only for windowing; Skia handles all rasterization and fonts.
//! Unlike the Cairo backend, Skia is self-contained — it rasterizes at CSS
//! pixels (DPR = 1), so no DPR sink is needed.
//!
//! Usage: cargo run -p example-browser --bin gtk4-skia -- https://example.com

use example_browser::common::engine::SetupOptions;
use example_browser::common::{logging, rt};
use example_browser::shell::gtk4::cpu::{run, Gtk4CpuOptions};
use std::sync::Arc;

fn main() {
    logging::init();
    rt::init("gosub-pipeline-rt");

    run(Gtk4CpuOptions {
        app_id: "io.gosub.gtk4-skia",
        window_title: "Gosub Browser — GTK4 + Skia",
        default_url: "https://stop-ai-slop.com",
        setup: SetupOptions {
            zone_uuid: uuid::uuid!("f1234567-abcd-4000-8000-00000000000b"),
            tab_title: "Gosub Skia",
            local_store_path: ".pipeline-browser-local.db",
            initial_viewport: None,
        },
        make_backend: || Arc::new(gosub_render_pipeline::render::backends::skia::SkiaBackend::new()),
        dpr_sink: None,
    });
}
