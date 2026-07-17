//! A text-mode browser: renders a real page into character cells and lets you scroll it.
//!
//! Proof-of-concept for the `gosub_renderer_tui` backend. It renders text, solid backgrounds and
//! borders; images, SVG and gradients are skipped. There is no clicking — the engine has no focus
//! model or event dispatch yet — so this is a reader, not a browser.
//!
//! Usage: `cargo run --release --example tui-browser -- [url] [light|dark]`
//!
//! **Use `--release`.** A debug build of the engine is roughly 9× slower and takes seconds to show
//! a page; the cost is in HTML parsing, render-tree building and layout, not in the cell
//! rasterizer (which is well under a millisecond either way).
//!
//! Keys: `q`/`Esc` quit · `↑`/`↓`/`j`/`k` line · `PgUp`/`PgDn`/`Space` page · `Home`/`End` ends ·
//! mouse wheel scrolls.
//!
//! Set `GOSUB_TUI_TIMING=1` to print the pipeline's stage timings on exit.

// Example code: panicking on bad input is the desired behavior, as in any test code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cow_utils::CowUtils;
use gosub_engine::events::{EngineEvent, NavigationEvent, TabCommand};
use gosub_engine::net::types::FetchResultMeta;
use gosub_engine::net::DecisionToken;
use gosub_engine::tab::{TabDefaults, TabHandle};
use gosub_engine::{
    cookies::DefaultCookieJar,
    storage::{InMemoryLocalStore, InMemorySessionStore, PartitionPolicy, StorageService},
    zone::{ZoneConfig, ZoneServices},
    Action, DefaultRenderConfig, EngineConfig, EngineError, GosubEngine, NavigationId,
};
use gosub_render_pipeline::render::{DefaultCompositor, Viewport};
use gosub_renderer_tui::{Background, CellCanvas, CellFontSystem, TuiBackend, CELL_H, CELL_W};
use parking_lot::Mutex;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::style::Color;
use ratatui::{DefaultTerminal, Frame};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::TryRecvError;

type TuiConfig = DefaultRenderConfig<TuiBackend, CellFontSystem, DefaultCompositor>;

/// Redraw cadence. The page keeps rasterizing in the background (late CSS, reflows), so the view
/// is rebuilt from the canvas on every tick rather than only on input.
const TICK: Duration = Duration::from_millis(33);
/// Rows per mouse-wheel notch.
const WHEEL_LINES: usize = 3;

#[tokio::main]
async fn main() -> Result<(), EngineError> {
    let mut args = std::env::args().skip(1);
    let url = args
        .next()
        .unwrap_or_else(|| "https://news.ycombinator.com".to_string());
    // Pages assume a light background; adapt to a dark terminal unless told otherwise.
    let background = match args.next().as_deref() {
        Some("light") => Background::Light,
        _ => Background::Dark,
    };

    // A user-agent stylesheet for text mode, at `User` origin with `!important` so it beats the
    // page's own rules. It targets the real source of vertical raggedness in a cell grid: `em`-based
    // block margins and padding, which scale with font-size and round to whole cells, stacking up
    // huge gaps around headings and centered boxes. (Line-height is *not* the culprit — this
    // pipeline sizes text lines from the font system, so CSS line-height barely moves the layout.)
    //
    // So: zero all box spacing, then restore exactly one blank cell-row after *content* blocks —
    // lynx-style. `div`/`table` are deliberately excluded; on table-based layouts like HN they are
    // scaffolding, not content, and spacing them makes the page taller instead of tighter.
    //
    // Respecting an existing value lets `GOSUB_USER_STYLESHEET=` (empty) turn it off for comparison.
    if std::env::var_os("GOSUB_USER_STYLESHEET").is_none() {
        std::env::set_var(
            "GOSUB_USER_STYLESHEET",
            format!(
                "* {{ margin: 0 !important; padding: 0 !important; }} \
                 p,h1,h2,h3,h4,h5,h6,ul,ol,blockquote,pre {{ margin-bottom: {}px !important; }}",
                CELL_H as u32
            ),
        );
    }

    let backend = TuiBackend::new();
    let canvas = backend.canvas();

    let engine_cfg = EngineConfig::builder()
        .max_zones(1)
        .build()
        .expect("invalid engine config");
    let mut engine = GosubEngine::<TuiConfig>::new(
        Some(engine_cfg),
        Arc::new(backend),
        Arc::new(DefaultCompositor::default()),
    );
    let engine_join_handle = tokio::spawn(engine.start().expect("cannot start engine"));
    let mut event_rx = engine.subscribe_events();

    let zone_services = ZoneServices {
        storage: Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new()),
        )),
        cookie_store: None,
        cookie_jar: Some(DefaultCookieJar::new().into()),
        partition_policy: PartitionPolicy::None,
    };
    let mut zone = engine.create_zone(
        Some(ZoneConfig::builder().build().expect("invalid zone config")),
        zone_services,
        None,
    )?;

    // `ratatui::init` enters the alternate screen, turns on raw mode, and installs a panic hook
    // that restores the terminal — without which a panic leaves the shell unusable.
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);

    let (cols, rows) = terminal_cells(&terminal);
    let tab = zone
        .create_tab(
            TabDefaults {
                url: None,
                title: Some("tui".into()),
                viewport: Some(Viewport::new(0, 0, cols * CELL_W as u32, rows * CELL_H as u32)),
            },
            None,
        )
        .await
        .expect("cannot create tab");
    // The tab worker only rebuilds the pipeline on its own tick, so this rate is also the
    // worst-case delay between the page becoming ready and it reaching the canvas.
    tab.send(TabCommand::ResumeDrawing { fps: 30 }).await?;
    tab.send(TabCommand::Navigate { url: url.clone() }).await?;

    // The UI runs on its own OS thread, deliberately *not* on the tokio runtime.
    //
    // The engine does ~550ms of synchronous CPU per pipeline pass inside an async task. That stops
    // the worker holding tokio's time driver from handing it off, so every timer in the runtime
    // stalls for the duration — measured: an interval on the runtime misses by ~570ms, while a
    // plain thread is unaffected. A UI on the runtime therefore freezes exactly when it has the
    // most to say. The winit/GTK examples avoid this by accident, since their event loops already
    // live on a native thread.
    let ui_canvas = canvas.clone();
    let ui_tab = tab.clone();
    let ui_url = url.clone();
    let ui = std::thread::spawn(move || {
        let result = run_ui(&mut terminal, &mut event_rx, &ui_tab, &ui_canvas, background, &ui_url);
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        ratatui::restore();
        result
    });
    let result = tokio::task::spawn_blocking(move || ui.join())
        .await
        .map(|joined| joined.unwrap_or(Ok(())))
        .unwrap_or(Ok(()));

    // Printed after the terminal is restored: a TUI owns stdout, so this is the only place the
    // pipeline's stage timings can actually be read.
    if std::env::var("GOSUB_TUI_TIMING").is_ok() {
        gosub_shared::timing::dump(false);
    }

    engine.shutdown().await?;
    if let Err(join_err) = engine_join_handle.await {
        eprintln!("engine task panicked: {join_err}");
    }
    result
}

/// Terminal size in character cells, minus the status row.
fn terminal_cells(terminal: &DefaultTerminal) -> (u32, u32) {
    let size = terminal.size().unwrap_or_default();
    (
        u32::from(size.width).max(1),
        u32::from(size.height).saturating_sub(1).max(1),
    )
}

/// The terminal UI loop. Synchronous by design — see the note at the call site.
fn run_ui(
    terminal: &mut DefaultTerminal,
    event_rx: &mut tokio::sync::broadcast::Receiver<EngineEvent>,
    tab: &TabHandle,
    canvas: &Arc<Mutex<CellCanvas>>,
    background: Background,
    url: &str,
) -> Result<(), EngineError> {
    let mut scroll = 0usize;
    // The page area is blank for ~1.5s on a real site: the fetch, then ~550ms per pipeline pass.
    // Both waits get a live spinner, because a static status line through a blank screen reads as
    // a hang — which is exactly what it looked like before this loop moved off the runtime.
    let mut phase = Phase::Loading;
    let started = std::time::Instant::now();
    let mut marks = Milestones::default();

    loop {
        // Engine events: drained without blocking, so a quiet engine never stalls the redraw.
        loop {
            match event_rx.try_recv() {
                Ok(ev) => handle_engine_event(ev, tab, &mut phase, &mut marks, started),
                // Lagged just means we missed some; the next frame re-reads the canvas anyway.
                Err(TryRecvError::Lagged(_)) => continue,
                Err(TryRecvError::Empty) | Err(TryRecvError::Closed) => break,
            }
        }

        let page_rows = usize::from(terminal.size().unwrap_or_default().height.saturating_sub(1)).max(1);
        let max_scroll = canvas.lock().rows().saturating_sub(page_rows);

        while event::poll(Duration::ZERO).unwrap_or(false) {
            let Ok(input) = event::read() else { break };
            match input {
                Event::Key(key) if is_quit(&key) => {
                    marks.report();
                    return Ok(());
                }
                Event::Key(KeyEvent { code, .. }) => match code {
                    KeyCode::Down | KeyCode::Char('j') => scroll = (scroll + 1).min(max_scroll),
                    KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
                    KeyCode::PageDown | KeyCode::Char(' ') => scroll = (scroll + page_rows).min(max_scroll),
                    KeyCode::PageUp => scroll = scroll.saturating_sub(page_rows),
                    KeyCode::Home => scroll = 0,
                    KeyCode::End => scroll = max_scroll,
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollDown => scroll = (scroll + WHEEL_LINES).min(max_scroll),
                    MouseEventKind::ScrollUp => scroll = scroll.saturating_sub(WHEEL_LINES),
                    _ => {}
                },
                Event::Resize(w, h) => {
                    // Re-lay-out at the new width. The canvas clears itself when the rasterizer is
                    // told a full rebuild is starting, so the host must not clear it here: that
                    // would blank the view until the next pass lands.
                    scroll = 0;
                    let _ = tab.cmd_tx.blocking_send(TabCommand::SetViewport {
                        x: 0,
                        y: 0,
                        width: u32::from(w).max(1) * CELL_W as u32,
                        height: u32::from(h).saturating_sub(1).max(1) * CELL_H as u32,
                    });
                }
                _ => {}
            }
        }

        scroll = scroll.min(max_scroll);
        let rows_total = canvas.lock().rows();
        if rows_total > 0 && marks.first_content.is_none() {
            marks.first_content = Some(started.elapsed());
        }

        // Content on the canvas is the only true signal that the wait is over: the engine has no
        // event for "the pipeline produced a frame".
        if let Phase::Rendering { url } = &phase {
            if rows_total > 0 {
                phase = Phase::Done { text: url.clone() };
            }
        }

        let line = phase.status_line(url, started.elapsed());
        let _ = terminal.draw(|frame| {
            draw(frame, &canvas.lock(), scroll, rows_total, background, &line);
        });

        std::thread::sleep(TICK);
    }
}

/// Wall-clock milestones from process start to first paint.
///
/// The engine's timing table covers parsing and the pipeline, but not the navigation that precedes
/// them — DNS, TLS, time-to-first-byte and the UA's `DecisionRequired` round-trip are all invisible
/// to it. When first paint is slower than the table can explain, the gap is in here.
#[derive(Default)]
struct Milestones {
    nav_started: Option<Duration>,
    decision: Option<Duration>,
    nav_finished: Option<Duration>,
    first_content: Option<Duration>,
}

impl Milestones {
    fn report(&self) {
        if std::env::var("GOSUB_TUI_TIMING").is_err() {
            return;
        }
        let ms = |d: Option<Duration>| match d {
            Some(d) => format!("{:>8.0?}", d),
            None => "       -".to_string(),
        };
        eprintln!("\n=== Milestones (wall clock from start) ===");
        eprintln!("navigation started   {}", ms(self.nav_started));
        // Only fires for content types the UA has to rule on; text/html renders without asking.
        if self.decision.is_some() {
            eprintln!(
                "decision requested   {}   <- response headers are in",
                ms(self.decision)
            );
        }
        eprintln!(
            "navigation finished  {}   <- DNS + TLS + TTFB + body + parse + stylesheets",
            ms(self.nav_finished)
        );
        eprintln!(
            "first paint          {}   <- + the first pipeline pass",
            ms(self.first_content)
        );
    }
}

/// Fold one engine event into the UI's phase. Only navigation matters here; the page itself is
/// read straight off the canvas.
fn handle_engine_event(
    ev: EngineEvent,
    tab: &TabHandle,
    phase: &mut Phase,
    marks: &mut Milestones,
    started: std::time::Instant,
) {
    match ev {
        EngineEvent::Navigation {
            event: NavigationEvent::Started { .. },
            ..
        } => {
            marks.nav_started.get_or_insert_with(|| started.elapsed());
        }
        EngineEvent::Navigation {
            event:
                NavigationEvent::DecisionRequired {
                    nav_id,
                    meta,
                    decision_token,
                },
            ..
        } => {
            marks.decision.get_or_insert_with(|| started.elapsed());
            on_decision_required(tab, nav_id, meta, decision_token)
        }
        // The document has arrived and parsed, but the pipeline hasn't run yet — so there is still
        // nothing on screen. Keep spinning until the canvas fills.
        EngineEvent::Navigation {
            event: NavigationEvent::Finished { url, .. },
            ..
        } => {
            marks.nav_finished.get_or_insert_with(|| started.elapsed());
            *phase = Phase::Rendering { url: url.to_string() };
        }
        EngineEvent::Navigation {
            event: NavigationEvent::Failed { url, error, .. },
            ..
        } => {
            *phase = Phase::Done {
                text: format!("FAILED {url} — {error}"),
            }
        }
        _ => {}
    }
}

/// What the browser is waiting on, and therefore what the status line should say.
enum Phase {
    /// Fetching and parsing the document.
    Loading,
    /// Document parsed; the pipeline hasn't produced a frame yet. Still a blank screen.
    Rendering { url: String },
    /// Something is on screen (or the navigation failed).
    Done { text: String },
}

impl Phase {
    fn status_line(&self, url: &str, elapsed: Duration) -> String {
        const SPINNER: [char; 4] = ['|', '/', '-', '\\'];
        let frame = SPINNER[(elapsed.as_millis() / 120) as usize % SPINNER.len()];
        let secs = elapsed.as_secs_f32();
        match self {
            Phase::Loading => format!("{frame} loading {url} — {secs:.1}s"),
            Phase::Rendering { url } => format!("{frame} rendering {url} — {secs:.1}s"),
            Phase::Done { text } => text.clone(),
        }
    }
}

fn is_quit(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
}

/// Blit the page-space canvas into the terminal, offset by `scroll`, with a status row beneath.
fn draw(
    frame: &mut Frame,
    canvas: &CellCanvas,
    scroll: usize,
    rows_total: usize,
    background: Background,
    status: &str,
) {
    let area = frame.area();
    let page_rows = area.height.saturating_sub(1);
    let buf = frame.buffer_mut();

    for y in 0..page_rows {
        for x in 0..area.width {
            let Some(target) = buf.cell_mut((x, y)) else {
                continue;
            };
            match canvas.cell(usize::from(x), scroll + usize::from(y)) {
                Some(cell) => {
                    if cell.ch == '\0' {
                        // Second half of a wide glyph: ratatui expects the trailing cell to carry
                        // an empty symbol so it doesn't advance twice.
                        target.set_symbol("");
                    } else {
                        target.set_char(cell.ch);
                    }
                    let (r, g, b) = background.adapt(cell.fg);
                    target.fg = Color::Rgb(r, g, b);
                    target.bg = match cell.bg.map(|c| background.adapt(c)) {
                        Some((r, g, b)) => Color::Rgb(r, g, b),
                        // Nothing painted: let the terminal's own background through.
                        None => Color::Reset,
                    };
                }
                None => {
                    target.set_char(' ');
                    target.fg = Color::Reset;
                    target.bg = Color::Reset;
                }
            }
        }
    }

    let end = (scroll + usize::from(page_rows)).min(rows_total);
    let bar = format!(" {status}  │  rows {}-{end} of {rows_total}  │  q quit ", scroll + 1);
    for x in 0..area.width {
        let Some(target) = buf.cell_mut((x, area.height.saturating_sub(1))) else {
            continue;
        };
        let ch = bar.chars().nth(usize::from(x)).unwrap_or(' ');
        target.set_char(ch);
        target.fg = Color::Black;
        target.bg = Color::Gray;
    }
}

fn on_decision_required(
    tab_handle: &TabHandle,
    nav_id: NavigationId,
    meta: FetchResultMeta,
    decision_token: DecisionToken,
) {
    let ct: String = meta
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let action = if let Some(disp) = meta.headers.get(http::header::CONTENT_DISPOSITION) {
        let s = disp.to_str().unwrap_or_default().cow_to_ascii_lowercase();
        if s.contains("attachment") {
            Action::Download {
                dest: "/tmp/tui-download.bin".into(),
            }
        } else {
            Action::Render
        }
    } else if ct.starts_with("text/") || ct == "application/json" {
        Action::Render
    } else {
        Action::Download {
            dest: "/tmp/tui-download.bin".into(),
        }
    };

    let _ = tab_handle.cmd_tx.blocking_send(TabCommand::SubmitDecision {
        nav_id,
        decision_token,
        action,
    });
}
