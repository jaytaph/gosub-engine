//! Shared shell code for the gosub example browsers.
//!
//! Each `[[bin]]` in this crate pairs a UI toolkit (gtk4/winit/egui) with a render
//! backend (cairo/skia/skia-gpu/vello). The toolkit shells live in [`shell`]; the
//! engine/zone/tab plumbing they all share lives in [`common`].

pub mod common;
pub mod shell;
