//! Networking functionality
//!
//! This module contains all the networking functionality for the browser. This is normally the
//! lowlevel implementation of the browser. The networking module is responsible for making HTTP
//! requests, parsing the response and returning the result to the caller.
//!
//! It also contains additional networking components like the DNS resolver.

extern crate gosub_config;
#[cfg(not(target_arch = "wasm32"))]
pub mod dns;
pub mod errors;
pub mod http;

pub mod decision;
pub mod emitter;

mod decision_hub;
mod events;
mod fetch;
mod fetcher;
mod fs_utils;
mod io_runtime;
mod pump;
mod req_ref_tracker;
mod router;
mod shared_body;
mod types;
mod utils;


#[doc(inline)]
/// Tab management and browsing context API.
pub use gosub_shared::engine::tab;
