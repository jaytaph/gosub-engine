//! Gosub Browser chrome mockup — winit + Skia CPU + softbuffer.
//!
//! No engine wired up. Just the browser UI shell.
//!
//! Usage: cargo run -p example-nautical-browser [-- path/to/background.png]

use skia_safe::{
    surfaces, AlphaType, Canvas, Color4f, ColorType, Contains, Font, FontMgr, FontStyle, Image,
    ImageInfo, Paint, PaintStyle, Path, Rect as SkRect,
};
use softbuffer::Surface as SbSurface;
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton as WinitMouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

// ── Layout constants ──────────────────────────────────────────────────────────

const TOOLBAR_H: f32 = 50.0;
const BTN_W: f32 = 30.0;
const BTN_H: f32 = 30.0;
const BTN_Y: f32 = (TOOLBAR_H - BTN_H) / 2.0;
const MARGIN: f32 = 8.0;
const BTN_GAP: f32 = 4.0;
const URL_BAR_LEFT: f32 = MARGIN + 3.0 * BTN_W + 2.0 * BTN_GAP + MARGIN;
const RIGHT_BLOCK: f32 = MARGIN + 3.0 * BTN_W + 2.0 * BTN_GAP + MARGIN;

// ── Hit zones ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum HitZone {
    None,
    Back,
    Forward,
    Refresh,
    UrlBar,
    Star,
    Shield,
    Hamburger,
}

fn layout(win_w: f32) -> [(SkRect, HitZone); 7] {
    let bx = |i: f32| MARGIN + i * (BTN_W + BTN_GAP);
    let rx = |i: f32| win_w - MARGIN - BTN_W - i * (BTN_W + BTN_GAP);
    let url_w = win_w - URL_BAR_LEFT - RIGHT_BLOCK;
    [
        (SkRect::from_xywh(bx(0.0), BTN_Y, BTN_W, BTN_H), HitZone::Back),
        (SkRect::from_xywh(bx(1.0), BTN_Y, BTN_W, BTN_H), HitZone::Forward),
        (SkRect::from_xywh(bx(2.0), BTN_Y, BTN_W, BTN_H), HitZone::Refresh),
        (
            SkRect::from_xywh(URL_BAR_LEFT, BTN_Y, url_w.max(0.0), BTN_H),
            HitZone::UrlBar,
        ),
        (SkRect::from_xywh(rx(2.0), BTN_Y, BTN_W, BTN_H), HitZone::Star),
        (SkRect::from_xywh(rx(1.0), BTN_Y, BTN_W, BTN_H), HitZone::Shield),
        (SkRect::from_xywh(rx(0.0), BTN_Y, BTN_W, BTN_H), HitZone::Hamburger),
    ]
}

fn hit_test(x: f32, y: f32, win_w: f32) -> HitZone {
    if !(0.0..TOOLBAR_H).contains(&y) {
        return HitZone::None;
    }
    for (r, z) in layout(win_w) {
        if r.contains(skia_safe::Point::new(x, y)) {
            return z;
        }
    }
    HitZone::None
}

// ── App ───────────────────────────────────────────────────────────────────────

struct BrowserApp {
    window: Option<Arc<Window>>,
    sb_surface: Option<SbSurface<Arc<Window>, Arc<Window>>>,
    surface_size: (u32, u32),
    url_input: String,
    addr_focused: bool,
    cursor: PhysicalPosition<f64>,
    hovered: HitZone,
    bg_image: Option<Image>,
}

impl BrowserApp {
    fn new(image_path: Option<&str>) -> Self {
        static EMBEDDED: &[u8] = include_bytes!("background.png");

        let bg_image = match image_path {
            Some(p) => {
                let bytes = std::fs::read(p).unwrap_or_else(|_| EMBEDDED.to_vec());
                let data = skia_safe::Data::new_copy(&bytes);
                Image::from_encoded(data)
            }
            None => {
                let data = skia_safe::Data::new_copy(EMBEDDED);
                Image::from_encoded(data)
            }
        };
        BrowserApp {
            window: None,
            sb_surface: None,
            surface_size: (0, 0),
            url_input: String::new(),
            addr_focused: false,
            cursor: PhysicalPosition::default(),
            hovered: HitZone::None,
            bg_image,
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn redraw(&mut self) {
        let Some(sb) = &mut self.sb_surface else {
            return;
        };
        let (win_w, win_h) = self.surface_size;
        if win_w == 0 || win_h == 0 {
            return;
        }
        let Ok(nw) = NonZeroU32::try_from(win_w) else {
            return;
        };
        let Ok(nh) = NonZeroU32::try_from(win_h) else {
            return;
        };
        if sb.resize(nw, nh).is_err() {
            return;
        }
        let Ok(mut buf) = sb.buffer_mut() else {
            return;
        };

        // Use explicit RGBA8888 so byte order is consistent on macOS and Linux.
        let info = ImageInfo::new(
            (win_w as i32, win_h as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let Some(mut skia_surf) = surfaces::raster(&info, None, None) else {
            return;
        };
        let canvas = skia_surf.canvas();

        draw_content(canvas, win_w as f32, win_h as f32, TOOLBAR_H, &self.bg_image);
        draw_toolbar(
            canvas,
            win_w as f32,
            TOOLBAR_H,
            &self.url_input,
            self.addr_focused,
            self.hovered,
        );

        // Blit RGBA8888 → softbuffer 0x00RRGGBB.
        if let Some(pixmap) = canvas.peek_pixels() {
            if let Some(bytes) = pixmap.bytes() {
                let stride = pixmap.row_bytes();
                for row in 0..win_h as usize {
                    for col in 0..win_w as usize {
                        let off = row * stride + col * 4;
                        let r = bytes[off] as u32;
                        let g = bytes[off + 1] as u32;
                        let b = bytes[off + 2] as u32;
                        buf[row * win_w as usize + col] = (r << 16) | (g << 8) | b;
                    }
                }
            }
        }

        buf.present().unwrap_or_default();
    }
}

impl ApplicationHandler for BrowserApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default()
            .with_title("Gosub Browser")
            .with_inner_size(LogicalSize::new(1280u32, 820u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer ctx");
        let surface = SbSurface::new(&ctx, window.clone()).expect("softbuffer surface");
        let size = window.inner_size();
        self.surface_size = (size.width, size.height);
        self.window = Some(window);
        self.sb_surface = Some(surface);
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.redraw(),

            WindowEvent::Resized(PhysicalSize { width, height }) => {
                if width > 0 && height > 0 {
                    self.surface_size = (width, height);
                    self.request_redraw();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                let new_hover = hit_test(
                    position.x as f32,
                    position.y as f32,
                    self.surface_size.0 as f32,
                );
                if new_hover != self.hovered {
                    self.hovered = new_hover;
                    self.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: WinitMouseButton::Left,
                ..
            } => {
                let zone = hit_test(
                    self.cursor.x as f32,
                    self.cursor.y as f32,
                    self.surface_size.0 as f32,
                );
                let prev = self.addr_focused;
                self.addr_focused = zone == HitZone::UrlBar;
                if self.addr_focused != prev {
                    self.request_redraw();
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
            } if self.addr_focused => {
                match &logical_key {
                    Key::Named(NamedKey::Enter | NamedKey::Escape) => {
                        self.addr_focused = false;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.url_input.pop();
                    }
                    _ => {
                        if let Some(t) = &text {
                            self.url_input.push_str(t.as_str());
                        }
                    }
                }
                self.request_redraw();
            }

            _ => {}
        }
    }
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

fn icon_color(hovered: bool) -> Color4f {
    let v = if hovered { 1.0_f32 } else { 0.72 };
    Color4f::new(v, v, v, 1.0)
}

fn btn_hover_bg() -> Paint {
    let mut p = Paint::new(Color4f::new(1.0, 1.0, 1.0, 0.12), None);
    p.set_anti_alias(true);
    p
}

fn stroke_paint(color: Color4f, width: f32) -> Paint {
    let mut p = Paint::new(color, None);
    p.set_anti_alias(true);
    p.set_style(PaintStyle::Stroke);
    p.set_stroke_width(width);
    p.set_stroke_cap(skia_safe::PaintCap::Round);
    p.set_stroke_join(skia_safe::PaintJoin::Round);
    p
}

// ── Content area ─────────────────────────────────────────────────────────────

fn draw_content(canvas: &Canvas, win_w: f32, win_h: f32, toolbar_h: f32, image: &Option<Image>) {
    let content = SkRect::from_xywh(0.0, toolbar_h, win_w, win_h - toolbar_h);
    match image {
        Some(img) => {
            let iw = img.width() as f32;
            let ih = img.height() as f32;
            let cw = win_w;
            let ch = win_h - toolbar_h;
            let scale = (cw / iw).max(ch / ih);
            let sw = iw * scale;
            let sh = ih * scale;
            let dst = SkRect::from_xywh((cw - sw) / 2.0, toolbar_h + (ch - sh) / 2.0, sw, sh);
            canvas.save();
            canvas.clip_rect(content, None, None);
            canvas.draw_image_rect(img, None, dst, &Paint::default());
            canvas.restore();
        }
        None => {
            canvas.draw_rect(
                content,
                &Paint::new(Color4f::new(0.094, 0.11, 0.14, 1.0), None),
            );
        }
    }
}

// ── Toolbar ───────────────────────────────────────────────────────────────────

fn draw_toolbar(canvas: &Canvas, win_w: f32, h: f32, url: &str, focused: bool, hovered: HitZone) {
    canvas.draw_rect(
        SkRect::from_xywh(0.0, 0.0, win_w, h),
        &Paint::new(Color4f::new(0.11, 0.126, 0.153, 1.0), None),
    );
    canvas.draw_rect(
        SkRect::from_xywh(0.0, h - 1.0, win_w, 1.0),
        &Paint::new(Color4f::new(0.0, 0.0, 0.0, 0.35), None),
    );

    let zones = layout(win_w);
    let rect_of = |z: HitZone| zones.iter().find(|(_, hz)| *hz == z).unwrap().0;

    draw_back(canvas, rect_of(HitZone::Back), hovered == HitZone::Back);
    draw_forward(canvas, rect_of(HitZone::Forward), hovered == HitZone::Forward);
    draw_refresh(canvas, rect_of(HitZone::Refresh), hovered == HitZone::Refresh);
    draw_url_bar(canvas, rect_of(HitZone::UrlBar), url, focused);
    draw_star(canvas, rect_of(HitZone::Star), hovered == HitZone::Star);
    draw_shield(canvas, rect_of(HitZone::Shield), hovered == HitZone::Shield);
    draw_hamburger(canvas, rect_of(HitZone::Hamburger), hovered == HitZone::Hamburger);
}

fn draw_back(canvas: &Canvas, rect: SkRect, hovered: bool) {
    if hovered {
        canvas.draw_round_rect(rect, 5.0, 5.0, &btn_hover_bg());
    }
    let (cx, cy) = (rect.center_x(), rect.center_y());
    let p = stroke_paint(icon_color(hovered), 2.0);
    let mut path = Path::new();
    path.move_to((cx + 5.0, cy - 7.5));
    path.line_to((cx - 5.0, cy));
    path.line_to((cx + 5.0, cy + 7.5));
    canvas.draw_path(&path, &p);
}

fn draw_forward(canvas: &Canvas, rect: SkRect, hovered: bool) {
    if hovered {
        canvas.draw_round_rect(rect, 5.0, 5.0, &btn_hover_bg());
    }
    let (cx, cy) = (rect.center_x(), rect.center_y());
    let p = stroke_paint(icon_color(hovered), 2.0);
    let mut path = Path::new();
    path.move_to((cx - 5.0, cy - 7.5));
    path.line_to((cx + 5.0, cy));
    path.line_to((cx - 5.0, cy + 7.5));
    canvas.draw_path(&path, &p);
}

fn draw_refresh(canvas: &Canvas, rect: SkRect, hovered: bool) {
    if hovered {
        canvas.draw_round_rect(rect, 5.0, 5.0, &btn_hover_bg());
    }
    let (cx, cy) = (rect.center_x(), rect.center_y());
    let r = 8.5_f32;
    let p = stroke_paint(icon_color(hovered), 2.0);

    // Arc 290° clockwise starting at -70°
    let oval = SkRect::from_xywh(cx - r, cy - r, r * 2.0, r * 2.0);
    let mut arc = Path::new();
    arc.add_arc(oval, -70.0, 290.0);
    canvas.draw_path(&arc, &p);

    // Arrowhead at end angle = -70 + 290 = 220°
    let end_rad = 220_f32.to_radians();
    let ex = cx + r * end_rad.cos();
    let ey = cy + r * end_rad.sin();
    // Clockwise tangent at 220° → direction 310°; backward → 130°; barbs at 130°±35°
    let barb = 5.5_f32;
    let a1 = 95_f32.to_radians();
    let a2 = 165_f32.to_radians();
    let mut arr = Path::new();
    arr.move_to((ex + barb * a1.cos(), ey + barb * a1.sin()));
    arr.line_to((ex, ey));
    arr.line_to((ex + barb * a2.cos(), ey + barb * a2.sin()));
    canvas.draw_path(&arr, &p);
}

fn draw_url_bar(canvas: &Canvas, rect: SkRect, url: &str, focused: bool) {
    let mut bg = Paint::new(Color4f::new(0.157, 0.18, 0.22, 1.0), None);
    bg.set_anti_alias(true);
    canvas.draw_round_rect(rect, 6.0, 6.0, &bg);

    if focused {
        canvas.draw_round_rect(
            rect,
            6.0,
            6.0,
            &stroke_paint(Color4f::new(0.25, 0.50, 0.95, 1.0), 1.5),
        );
    }

    // Search icon
    let sx = rect.left + 12.0;
    let sy = rect.center_y();
    let sr = 5.0_f32;
    let ic = stroke_paint(Color4f::new(0.55, 0.58, 0.63, 1.0), 1.5);
    canvas.draw_circle((sx, sy), sr, &ic);
    let d = sr * 0.7;
    canvas.draw_line((sx + d, sy + d), (sx + d + sr * 0.6, sy + d + sr * 0.6), &ic);

    // Text
    let display = if url.is_empty() {
        "Search or enter address"
    } else {
        url
    };
    let text_color = if url.is_empty() {
        Color4f::new(0.45, 0.48, 0.53, 1.0)
    } else {
        Color4f::new(0.88, 0.89, 0.91, 1.0)
    };

    thread_local! {
        static FONT_MGR: FontMgr = FontMgr::new();
    }
    let typeface = FONT_MGR.with(|fm| {
        fm.legacy_make_typeface(None, FontStyle::normal()).unwrap_or_else(|| {
            fm.legacy_make_typeface("sans-serif", FontStyle::normal())
                .expect("typeface")
        })
    });
    let font = Font::new(typeface, 13.5);
    let mut tp = Paint::new(text_color, None);
    tp.set_anti_alias(true);
    let tx = rect.left + 26.0;
    let ty = rect.center_y() + 5.0;
    canvas.draw_str(display, (tx, ty), &font, &tp);

    if focused && !display.is_empty() {
        let (_, bounds) = font.measure_str(display, Some(&tp));
        canvas.draw_line(
            (tx + bounds.width() + 2.0, rect.top + 7.0),
            (tx + bounds.width() + 2.0, rect.bottom - 7.0),
            &stroke_paint(Color4f::new(0.88, 0.89, 0.91, 1.0), 1.5),
        );
    }
}

fn draw_star(canvas: &Canvas, rect: SkRect, hovered: bool) {
    if hovered {
        canvas.draw_round_rect(rect, 5.0, 5.0, &btn_hover_bg());
    }
    let (cx, cy) = (rect.center_x(), rect.center_y());
    let p = stroke_paint(icon_color(hovered), 1.5);
    let outer = 9.5_f32;
    let inner = 4.0_f32;
    let mut path = Path::new();
    for i in 0..5_u32 {
        let ao = ((i as f32 * 72.0) - 90.0).to_radians();
        let ai = ((i as f32 * 72.0 + 36.0) - 90.0).to_radians();
        let op = (cx + outer * ao.cos(), cy + outer * ao.sin());
        let ip_pt = (cx + inner * ai.cos(), cy + inner * ai.sin());
        if i == 0 {
            path.move_to(op);
        } else {
            path.line_to(op);
        }
        path.line_to(ip_pt);
    }
    path.close();
    canvas.draw_path(&path, &p);
}

fn draw_shield(canvas: &Canvas, rect: SkRect, hovered: bool) {
    if hovered {
        canvas.draw_round_rect(rect, 5.0, 5.0, &btn_hover_bg());
    }
    let (cx, cy) = (rect.center_x(), rect.center_y());
    let p = stroke_paint(icon_color(hovered), 1.5);
    let top = cy - 10.0;
    let bot = cy + 11.0;
    let hw = 10.0_f32;
    let mut path = Path::new();
    path.move_to((cx, top));
    path.line_to((cx + hw, top + 3.0));
    path.line_to((cx + hw, cy + 1.0));
    path.cubic_to((cx + hw, cy + 7.0), (cx, bot), (cx, bot));
    path.cubic_to((cx, bot), (cx - hw, cy + 7.0), (cx - hw, cy + 1.0));
    path.line_to((cx - hw, top + 3.0));
    path.close();
    canvas.draw_path(&path, &p);
}

fn draw_hamburger(canvas: &Canvas, rect: SkRect, hovered: bool) {
    if hovered {
        canvas.draw_round_rect(rect, 5.0, 5.0, &btn_hover_bg());
    }
    let (cx, cy) = (rect.center_x(), rect.center_y());
    let p = stroke_paint(icon_color(hovered), 2.0);
    let hw = 8.0_f32;
    for dy in [-5.5_f32, 0.0, 5.5] {
        canvas.draw_line((cx - hw, cy + dy), (cx + hw, cy + dy), &p);
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .env()
        .init()
        .unwrap_or_default();

    let image_path = std::env::args().nth(1);
    let mut app = BrowserApp::new(image_path.as_deref());
    EventLoop::new()
        .expect("event loop")
        .run_app(&mut app)
        .expect("run app");
}
