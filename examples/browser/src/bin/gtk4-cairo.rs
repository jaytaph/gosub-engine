//! Browser binary: GTK4 toolkit + Cairo (CPU) rasterizer.
//!
//! Usage: cargo run -p example-browser --bin gtk4-cairo -- https://example.com

use example_browser::common::engine::SetupOptions;
use example_browser::common::{logging, rt};
use example_browser::shell::gtk4::cpu::{run, Gtk4CpuOptions};
use std::sync::Arc;

fn main() {
    logging::init();
    rt::init("gosub-pipeline-rt");

    run(Gtk4CpuOptions {
        app_id: "io.gosub.pipeline-browser",
        window_title: "Gosub Browser — GTK4 + Cairo",
        default_url: "https://stop-ai-slop.com",
        setup: SetupOptions {
            zone_uuid: uuid::uuid!("f1234567-abcd-4000-8000-000000000001"),
            tab_title: "Pipeline Browser",
            local_store_path: ".pipeline-browser-local.db",
            // No initial viewport — let connect_resize set it with the correct DPR.
            // If we pre-set a viewport here (DPR=1), the engine won't recreate the
            // surface when connect_resize sends the same CSS dimensions with DPR=2.
            initial_viewport: None,
        },
        make_backend: || Arc::new(gosub_render_pipeline::render::backends::cairo::CairoBackend::new()),
        dpr_sink: Some(|scale| {
            gosub_render_pipeline::render::backends::cairo::DEVICE_PIXEL_RATIO
                .store(scale, std::sync::atomic::Ordering::Relaxed);
        }),
    });
}
