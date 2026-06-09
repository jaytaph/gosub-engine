//! Minimal browser window: Skia GPU (Metal/Ganesh) compositor + winit.
//!
//! macOS only. Usage: cargo run -p example-winit-skia-gpu-metal -- https://example.com
//!
//! The engine rasterizes tiles on worker threads using SkiaRasterizer (CPU).
//! The main (event-loop) thread receives a TileCache and composites the tiles
//! directly onto the Metal window surface via Skia's Ganesh GPU backend — no CPU
//! readback required.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("winit-skia-gpu-metal: this example requires macOS");
    std::process::exit(1);
}

// ── macOS-only implementation ─────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod metal_app {
    use cocoa::appkit::NSView;
    use cocoa::base::{id as CocoaId, YES};
    use core_graphics_types::geometry::CGSize;
    use gosub_engine::events::{EngineEvent, MouseButton, NavigationEvent, TabCommand};
    use gosub_engine::storage::{InMemorySessionStore, PartitionPolicy, SqliteLocalStore, StorageService};
    use gosub_engine::tab::{TabDefaults, TabHandle, TabId};
    use gosub_engine::zone::{Zone, ZoneConfig, ZoneId, ZoneServices};
    use gosub_engine::GosubEngine;
    use gosub_render_pipeline::render::backend::{CachedTile, ExternalHandle};
    use gosub_render_pipeline::render::DefaultCompositor;
    use metal::{CommandQueue, Device, MetalLayer};
    use once_cell::sync::Lazy;
    use parking_lot::RwLock;
    use skia_safe::gpu::ganesh::surface_ganesh;
    use skia_safe::gpu::{self, mtl, DirectContext, SurfaceOrigin};
    use skia_safe::{Color4f, ColorType, Font, FontMgr, FontStyle, ImageInfo, Paint, Rect as SkRect};
    use std::sync::Arc;
    use tokio::runtime::{Builder, Runtime};
    use url::Url;
    use uuid::uuid;
    use winit::application::ApplicationHandler;
    use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
    use winit::event::{ElementState, KeyEvent, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
    use winit::keyboard::{Key, NamedKey};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::{Window, WindowAttributes, WindowId};

    const DEFAULT_ZONE: uuid::Uuid = uuid!("f1234567-abcd-4000-8000-00000000000e");
    const ADDRESS_BAR_HEIGHT: f32 = 36.0;
    const SCROLL_MULTIPLIER: f32 = 12.5;

    pub static TOKIO_RT: Lazy<Runtime> = Lazy::new(|| {
        Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .thread_name("gosub-winit-skia-metal-rt")
            .build()
            .expect("tokio runtime")
    });

    // ── Metal state kept on the main thread ──────────────────────────────────────

    pub struct MetalState {
        // `layer` and `context`/`queue` are accessed in the same redraw() call:
        // `layer` via an immutable borrow (drawable), `context` via a mutable borrow.
        // NLL handles disjoint field borrows, so no wrapper struct is needed.
        layer: MetalLayer,
        #[allow(dead_code)]
        device: Device,
        queue: CommandQueue,
        context: DirectContext,
    }

    impl MetalState {
        pub fn new(window: &Window) -> Self {
            let device = Device::system_default().expect("no Metal device");
            let queue = device.new_command_queue();

            let layer = MetalLayer::new();
            layer.set_device(&device);
            layer.set_pixel_format(metal::MTLPixelFormat::BGRA8Unorm);
            layer.set_presents_with_transaction(false);
            // Allow Skia to read from the texture (required for some Ganesh ops).
            layer.set_framebuffer_only(false);

            // Attach the CAMetalLayer to the winit NSView.
            let raw = window.window_handle().expect("window handle").as_raw();
            if let RawWindowHandle::AppKit(appkit) = raw {
                unsafe {
                    let ns_view: CocoaId = appkit.ns_view.as_ptr() as _;
                    let layer_ptr = layer.as_ptr() as CocoaId;
                    ns_view.setLayer(layer_ptr);
                    ns_view.setWantsLayer(YES);
                }
            }

            let backend = unsafe {
                mtl::BackendContext::new(
                    device.as_ptr() as mtl::Handle,
                    queue.as_ptr() as mtl::Handle,
                    std::ptr::null(),
                )
            };
            let context = gpu::direct_contexts::make_metal(&backend, None)
                .expect("Skia Metal DirectContext");

            MetalState { layer, device, queue, context }
        }

        pub fn resize(&self, width: u32, height: u32) {
            self.layer.set_drawable_size(CGSize::new(width as f64, height as f64));
        }
    }

    // ── Application ───────────────────────────────────────────────────────────────

    pub struct BrowserApp {
        #[allow(dead_code)]
        engine: GosubEngine,
        #[allow(dead_code)]
        zone: Zone,
        tab: TabHandle,
        tab_id: TabId,
        compositor: Arc<RwLock<DefaultCompositor>>,
        #[allow(dead_code)]
        proxy: EventLoopProxy<()>,

        window: Option<Arc<Window>>,
        metal: Option<MetalState>,
        surface_size: (u32, u32),

        url_input: String,
        addr_focused: bool,
        cursor: PhysicalPosition<f64>,
        scroll: (f32, f32),
        page_height: f32,
        viewport: (u32, u32),
    }

    impl BrowserApp {
        fn navigate(&mut self) {
            let mut s = self.url_input.clone();
            if !s.contains("://") {
                s = format!("https://{s}");
                self.url_input = s.clone();
            }
            let Ok(_) = Url::parse(&s) else { return };
            self.scroll = (0.0, 0.0);
            let tab = self.tab.clone();
            TOKIO_RT.spawn(async move {
                let _ = tab.send(TabCommand::Navigate { url: s }).await;
                let _ = tab.send(TabCommand::ResumeDrawing { fps: 30 }).await;
            });
        }

        fn redraw(&mut self) {
            let (win_w, win_h) = self.surface_size;
            if win_w == 0 || win_h == 0 {
                return;
            }
            let Some(metal) = self.metal.as_mut() else { return };

            // NLL field-borrow splitting: `metal.layer` is borrowed immutably (via
            // `drawable`) while `metal.context` is borrowed mutably for Skia below.
            let Some(drawable) = metal.layer.next_drawable() else { return };

            let tex_ptr = drawable.texture().as_ptr() as mtl::Handle;
            let info = unsafe { mtl::TextureInfo::new(tex_ptr) };
            let render_target =
                gpu::backend_render_targets::make_mtl((win_w as i32, win_h as i32), &info);

            let Some(mut skia_surface) = surface_ganesh::wrap_backend_render_target(
                &mut metal.context,
                &render_target,
                SurfaceOrigin::TopLeft,
                ColorType::BGRA8888,
                None,
                None,
            ) else {
                return;
            };

            let canvas = skia_surface.canvas();
            canvas.clear(Color4f::new(1.0, 1.0, 1.0, 1.0));

            let content_h = win_h.saturating_sub(ADDRESS_BAR_HEIGHT as u32);
            {
                let guard = self.compositor.read();
                if let Some(handle) = guard.frame_for(self.tab_id) {
                    composite_tiles(
                        canvas,
                        win_w,
                        ADDRESS_BAR_HEIGHT,
                        content_h,
                        &handle,
                        &mut self.page_height,
                    );
                }
            }

            draw_address_bar(canvas, win_w, ADDRESS_BAR_HEIGHT as i32, &self.url_input, self.addr_focused);

            drop(skia_surface);

            // Flush Skia and present the drawable via a Metal command buffer.
            metal.context.flush_and_submit();
            let cmd = metal.queue.new_command_buffer();
            cmd.present_drawable(drawable);
            cmd.commit();
        }

        fn is_addr_bar(&self, y: f64) -> bool {
            y < ADDRESS_BAR_HEIGHT as f64
        }
        fn css_x(&self, x: f64) -> f32 {
            (x + self.scroll.0 as f64) as f32
        }
        fn css_y(&self, y: f64) -> f32 {
            (y - ADDRESS_BAR_HEIGHT as f64 + self.scroll.1 as f64) as f32
        }
    }

    impl ApplicationHandler<()> for BrowserApp {
        fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

        fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => self.redraw(),

                WindowEvent::Resized(PhysicalSize { width, height }) => {
                    if width == 0 || height == 0 {
                        return;
                    }
                    self.surface_size = (width, height);
                    let content_h = height.saturating_sub(ADDRESS_BAR_HEIGHT as u32);
                    self.viewport = (width, content_h);
                    self.scroll = (0.0, 0.0);
                    if let Some(metal) = &self.metal {
                        metal.resize(width, height);
                    }
                    let tab = self.tab.clone();
                    TOKIO_RT.spawn(async move {
                        let _ = tab
                            .send(TabCommand::SetViewport {
                                x: 0,
                                y: 0,
                                width,
                                height: content_h,
                            })
                            .await;
                    });
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                WindowEvent::CursorMoved { position, .. } => {
                    self.cursor = position;
                    if !self.is_addr_bar(position.y) {
                        let (x, y) = (self.css_x(position.x), self.css_y(position.y));
                        let tab = self.tab.clone();
                        TOKIO_RT.spawn(async move {
                            let _ = tab.send(TabCommand::MouseMove { x, y }).await;
                        });
                    }
                }

                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: WinitMouseButton::Left,
                    ..
                } => {
                    if self.is_addr_bar(self.cursor.y) {
                        self.addr_focused = true;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    } else {
                        self.addr_focused = false;
                        let (x, y) = (self.css_x(self.cursor.x), self.css_y(self.cursor.y));
                        let tab = self.tab.clone();
                        TOKIO_RT.spawn(async move {
                            let _ = tab
                                .send(TabCommand::MouseDown {
                                    x,
                                    y,
                                    button: MouseButton::Left,
                                })
                                .await;
                        });
                    }
                }

                WindowEvent::MouseWheel { delta, .. } => {
                    let (dx, dy) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x * SCROLL_MULTIPLIER, y * SCROLL_MULTIPLIER),
                        MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                    };
                    let max_y = (self.page_height - self.viewport.1 as f32).max(0.0);
                    self.scroll.0 = (self.scroll.0 + dx).max(0.0);
                    self.scroll.1 = (self.scroll.1 + dy).clamp(0.0, max_y);
                    let tab = self.tab.clone();
                    TOKIO_RT.spawn(async move {
                        let _ = tab
                            .send(TabCommand::MouseScroll {
                                delta_x: dx,
                                delta_y: dy,
                            })
                            .await;
                    });
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            logical_key,
                            text,
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } if self.addr_focused => match &logical_key {
                    Key::Named(NamedKey::Enter) => self.navigate(),
                    Key::Named(NamedKey::Escape) => {
                        self.addr_focused = false;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.url_input.pop();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                    _ => {
                        if let Some(t) = &text {
                            self.url_input.push_str(t.as_str());
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                    }
                },

                _ => {}
            }
        }
    }

    // ── GPU tile compositing ───────────────────────────────────────────────────────

    fn composite_tiles(
        canvas: &skia_safe::Canvas,
        win_w: u32,
        addr_h: f32,
        content_h: u32,
        handle: &ExternalHandle,
        page_height: &mut f32,
    ) {
        let ExternalHandle::TileCache {
            tiles,
            page_height: ph,
            scroll_x: sx,
            scroll_y: sy,
            ..
        } = handle
        else {
            return;
        };
        *page_height = *ph;

        canvas.save();
        canvas.clip_rect(
            SkRect::from_xywh(0.0, addr_h, win_w as f32, content_h as f32),
            None,
            None,
        );

        for tile in tiles.iter() {
            let screen_x = tile.page_x - sx;
            let screen_y = tile.page_y - sy + addr_h;

            if screen_x + tile.width as f32 <= 0.0 { continue; }
            if screen_y + tile.height as f32 <= addr_h { continue; }
            if screen_x >= win_w as f32 { continue; }
            if screen_y >= addr_h + content_h as f32 { continue; }

            blit_tile(canvas, tile, screen_x, screen_y);
        }

        canvas.restore();
    }

    fn blit_tile(canvas: &skia_safe::Canvas, tile: &CachedTile, x: f32, y: f32) {
        let info = ImageInfo::new(
            (tile.width as i32, tile.height as i32),
            skia_safe::ColorType::BGRA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        if let Some(image) = skia_safe::images::raster_from_data(
            &info,
            skia_safe::Data::new_copy(&tile.data),
            (tile.width * 4) as usize,
        ) {
            canvas.draw_image(&image, (x, y), None);
        }
    }

    // ── Address bar ───────────────────────────────────────────────────────────────

    fn draw_address_bar(canvas: &skia_safe::Canvas, win_w: u32, h: i32, url: &str, focused: bool) {
        let w = win_w as f32;
        let hf = h as f32;

        let bg = if focused {
            Color4f::new(0.98, 0.98, 0.98, 1.0)
        } else {
            Color4f::new(0.93, 0.93, 0.93, 1.0)
        };
        let mut paint = Paint::new(bg, None);
        canvas.draw_rect(SkRect::from_xywh(0.0, 0.0, w, hf), &paint);

        let field_bg = if focused {
            Color4f::new(1.0, 1.0, 1.0, 1.0)
        } else {
            Color4f::new(0.97, 0.97, 0.97, 1.0)
        };
        paint.set_color4f(field_bg, None);
        paint.set_anti_alias(true);
        canvas.draw_round_rect(SkRect::from_xywh(6.0, 5.0, w - 12.0, hf - 10.0), 4.0, 4.0, &paint);

        let border = if focused {
            Color4f::new(0.26, 0.52, 0.96, 1.0)
        } else {
            Color4f::new(0.7, 0.7, 0.7, 1.0)
        };
        paint.set_color4f(border, None);
        paint.set_style(skia_safe::PaintStyle::Stroke);
        paint.set_stroke_width(1.0);
        canvas.draw_round_rect(SkRect::from_xywh(6.5, 5.5, w - 13.0, hf - 11.0), 4.0, 4.0, &paint);

        thread_local! { static FONT_MGR: FontMgr = FontMgr::new(); }
        let typeface = FONT_MGR.with(|fm| {
            fm.legacy_make_typeface(None, FontStyle::normal()).unwrap_or_else(|| {
                fm.legacy_make_typeface("sans-serif", FontStyle::normal())
                    .expect("typeface")
            })
        });
        let font = Font::new(typeface, 14.0);
        paint.set_color4f(Color4f::new(0.0, 0.0, 0.0, 1.0), None);
        paint.set_style(skia_safe::PaintStyle::Fill);
        canvas.draw_str(url, (12.0f32, hf - 10.0), &font, &paint);
    }

    // ── Entry point ───────────────────────────────────────────────────────────────

    pub fn run() {
        simple_logger::SimpleLogger::new()
            .with_level(log::LevelFilter::Warn)
            .env()
            .init()
            .unwrap_or_default();

        let initial_url = {
            let raw = std::env::args()
                .nth(1)
                .unwrap_or_else(|| "https://example.com".to_string());
            if raw.contains("://") { raw } else { format!("https://{raw}") }
        };

        let _rt_guard = TOKIO_RT.enter();
        let event_loop = EventLoop::<()>::with_user_event().build().expect("event loop");
        let proxy = event_loop.create_proxy();

        // Create the window (no glutin needed — Metal attaches directly to the NSView).
        let win_attrs = WindowAttributes::default()
            .with_title("Gosub Browser — winit + Skia Metal")
            .with_inner_size(LogicalSize::new(1024u32, 768u32));
        let window = event_loop.create_window(win_attrs).expect("window");

        let size = window.inner_size();
        let metal_state = MetalState::new(&window);
        metal_state.resize(size.width, size.height);

        // Engine + compositor
        let compositor = Arc::new(RwLock::new(DefaultCompositor::new({
            let p = proxy.clone();
            move || { let _ = p.send_event(()); }
        })));

        let backend = gosub_render_pipeline::render::backends::null::NullBackend::new()
            .expect("NullBackend");
        let mut engine = GosubEngine::new(None, Arc::new(backend), compositor.clone());
        let _join = engine.start().expect("engine start");

        let proxy_ev = proxy.clone();
        let mut event_rx = engine.subscribe_events();
        TOKIO_RT.spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(EngineEvent::Navigation {
                        event: NavigationEvent::Finished { .. } | NavigationEvent::Started { .. },
                        ..
                    }) => { let _ = proxy_ev.send_event(()); }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let zone_cfg = ZoneConfig::builder().do_not_track(true).build().expect("ZoneConfig");
        let zone_services = ZoneServices {
            storage: Arc::new(StorageService::new(
                Arc::new(SqliteLocalStore::new(":memory:").expect("local store")),
                Arc::new(InMemorySessionStore::new()),
            )),
            cookie_store: None,
            cookie_jar: None,
            partition_policy: PartitionPolicy::None,
        };

        let mut zone = engine
            .create_zone(zone_cfg, zone_services, Some(ZoneId::from(DEFAULT_ZONE)))
            .expect("zone");
        let tab = TOKIO_RT
            .block_on(zone.create_tab(
                TabDefaults {
                    url: None,
                    title: Some("Gosub".to_string()),
                    viewport: None,
                },
                None,
            ))
            .expect("tab");

        let tab_id = tab.tab_id;
        let nav_tab = tab.clone();
        let nav_url = initial_url.clone();
        TOKIO_RT.spawn(async move {
            let _ = nav_tab.send(TabCommand::Navigate { url: nav_url }).await;
        });

        let content_h = size.height.saturating_sub(ADDRESS_BAR_HEIGHT as u32);
        {
            let t = tab.clone();
            TOKIO_RT.block_on(async move {
                let _ = t.send(TabCommand::SetViewport {
                    x: 0, y: 0,
                    width: size.width,
                    height: content_h,
                }).await;
                let _ = t.send(TabCommand::ResumeDrawing { fps: 30 }).await;
            });
        }

        let window = Arc::new(window);

        let mut app = BrowserApp {
            engine,
            zone,
            tab,
            tab_id,
            compositor,
            proxy,
            window: Some(window),
            metal: Some(metal_state),
            surface_size: (size.width, size.height),
            url_input: initial_url,
            addr_focused: false,
            cursor: PhysicalPosition::default(),
            scroll: (0.0, 0.0),
            page_height: 0.0,
            viewport: (size.width, content_h),
        };

        event_loop.run_app(&mut app).expect("event loop");
    }
}

#[cfg(target_os = "macos")]
fn main() {
    metal_app::run();
}
