//! Browser binary: egui toolkit + Cairo (CPU) rasterizer.
//!
//! Cairo/Pango need GTK4 initialised for font rendering (no GTK window is created).
//! On headless systems set GDK_BACKEND=offscreen.
//!
//! Usage: cargo run -p example-browser --bin egui-cairo -- https://example.com

use example_browser::common::engine::SetupOptions;
use example_browser::common::{logging, nav, rt};
use example_browser::shell::egui::cpu::{run, EguiCpuOptions};
use std::sync::Arc;

fn main() -> Result<(), eframe::Error> {
    logging::init();
    rt::init("gosub-egui-cairo-rt");

    let initial_url = nav::normalize_url(&nav::initial_url_from_args("https://example.com"));
    run(
        initial_url,
        EguiCpuOptions {
            window_title: "Gosub Browser — egui + Cairo",
            setup: SetupOptions {
                zone_uuid: uuid::uuid!("f1234567-abcd-4000-8000-000000000005"),
                tab_title: "Gosub",
                local_store_path: ":memory:",
                initial_viewport: None,
            },
            make_backend: || Arc::new(gosub_render_pipeline::render::backends::cairo::CairoBackend::new()),
            init_gtk: true,
            dpr_sink: Some(|dpr| {
                gosub_render_pipeline::render::backends::cairo::DEVICE_PIXEL_RATIO
                    .store(dpr, std::sync::atomic::Ordering::Relaxed);
            }),
        },
    )
}
