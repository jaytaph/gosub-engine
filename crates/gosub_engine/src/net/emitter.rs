//! Observers the engine attaches to the net stack.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub use gosub_sonar::net::observer::NetObserver;

/// Whether response body previews are captured.
///
/// Off by default. A preview costs a copy of the net stack's peek window per request, which
/// is pure waste on every page where nobody is looking at a developer panel -- so the shell
/// turns it on when it opens one and off when it closes it, and the net stack skips the copy
/// entirely in between.
static CAPTURE_BODY_PREVIEWS: AtomicBool = AtomicBool::new(false);

/// How much of any single body is worth keeping. A megabyte covers documents, stylesheets
/// and scripts -- the things anyone actually reads in a panel -- without a single response
/// being able to cost much.
const DEFAULT_BODY_CAPTURE_LIMIT: usize = 1024 * 1024;

static BODY_CAPTURE_LIMIT: AtomicUsize = AtomicUsize::new(DEFAULT_BODY_CAPTURE_LIMIT);

/// Turn body capture on or off. See [`CAPTURE_BODY_PREVIEWS`].
pub fn set_capture_body_previews(enabled: bool) {
    CAPTURE_BODY_PREVIEWS.store(enabled, Ordering::Relaxed);
}

/// Whether body capture is currently on.
pub fn capture_body_previews() -> bool {
    CAPTURE_BODY_PREVIEWS.load(Ordering::Relaxed)
}

/// Change the per-response cap. Zero disables capture as surely as switching it off.
pub fn set_body_capture_limit(bytes: usize) {
    BODY_CAPTURE_LIMIT.store(bytes, Ordering::Relaxed);
}

/// The per-response cap.
pub fn body_capture_limit() -> usize {
    BODY_CAPTURE_LIMIT.load(Ordering::Relaxed)
}

/// Whether a response with these headers is worth capturing at all.
///
/// Refused before a byte is copied, which is the whole point: deciding afterwards would mean
/// having already paid for it.
///
/// - **Media** is never captured. A video is streamed, usually range-requested, and often
///   larger than memory; browsers refuse this too, which is why clicking a video in any
///   inspector shows no body.
/// - **Attachments** are downloads. The bytes are going to a file the user can open; holding
///   a second copy in the panel buys nothing.
/// - **Anything that declares itself over the cap** is refused up front rather than copied
///   to the cap and then shown truncated -- the declared length is free information and it
///   is better to say "too large" than to show a misleading first megabyte.
///
/// A response with no declared length is still captured: chunked encoding is exactly the
/// case where the cap has to do the work, and the tee stops at it.
pub(crate) fn should_capture_body(headers: &http::HeaderMap, content_length: Option<u64>) -> Option<usize> {
    capture_decision(capture_body_previews(), body_capture_limit(), headers, content_length)
}

/// The decision itself, with the switches passed in.
///
/// Split from [`should_capture_body`] so it can be tested without touching process-wide
/// state -- two tests flipping the same global is a race, and one that only shows up when
/// the suite runs in parallel.
fn capture_decision(
    enabled: bool,
    limit: usize,
    headers: &http::HeaderMap,
    content_length: Option<u64>,
) -> Option<usize> {
    if !enabled || limit == 0 {
        return None;
    }

    if let Some(value) = headers.get(http::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        use cow_utils::CowUtils;
        let kind = value.split(';').next().unwrap_or("").trim().cow_to_ascii_lowercase();
        if kind.starts_with("video/") || kind.starts_with("audio/") {
            return None;
        }
    }

    if let Some(value) = headers
        .get(http::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
    {
        use cow_utils::CowUtils;
        if value.trim_start().cow_to_ascii_lowercase().starts_with("attachment") {
            return None;
        }
    }

    if content_length.is_some_and(|len| len > limit as u64) {
        return None;
    }

    Some(limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;

    const LIMIT: usize = DEFAULT_BODY_CAPTURE_LIMIT;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        map
    }

    #[test]
    fn nothing_is_captured_while_the_switch_is_off() {
        let h = headers(&[("content-type", "text/html")]);
        assert_eq!(capture_decision(false, LIMIT, &h, Some(10)), None);
        // A zero budget is the same answer by another route.
        assert_eq!(capture_decision(true, 0, &h, Some(10)), None);
    }

    #[test]
    fn ordinary_documents_are_captured() {
        let h = headers(&[("content-type", "text/html; charset=utf-8")]);
        assert_eq!(capture_decision(true, LIMIT, &h, Some(4096)), Some(LIMIT));
    }

    #[test]
    fn a_response_with_no_declared_length_is_still_captured() {
        // Chunked: the cap is exactly what has to do the work here.
        let h = headers(&[("content-type", "text/css")]);
        assert_eq!(capture_decision(true, LIMIT, &h, None), Some(LIMIT));
    }

    #[test]
    fn media_is_never_captured() {
        for kind in ["video/mp4", "audio/mpeg", "VIDEO/WEBM; codecs=vp9"] {
            let h = headers(&[("content-type", kind)]);
            assert_eq!(
                capture_decision(true, LIMIT, &h, Some(1024)),
                None,
                "{kind} should be refused"
            );
        }
    }

    #[test]
    fn attachments_are_never_captured() {
        let h = headers(&[
            ("content-type", "application/pdf"),
            ("content-disposition", "attachment; filename=\"report.pdf\""),
        ]);
        assert_eq!(capture_decision(true, LIMIT, &h, Some(1024)), None);
    }

    #[test]
    fn an_inline_disposition_is_not_an_attachment() {
        let h = headers(&[("content-type", "text/plain"), ("content-disposition", "inline")]);
        assert_eq!(capture_decision(true, LIMIT, &h, Some(1024)), Some(LIMIT));
    }

    #[test]
    fn a_response_that_declares_itself_too_large_is_refused_up_front() {
        let h = headers(&[("content-type", "text/plain")]);
        assert_eq!(capture_decision(true, LIMIT, &h, Some(LIMIT as u64 + 1)), None);
        // Exactly at the cap is fine: the tee will fit it.
        assert_eq!(capture_decision(true, LIMIT, &h, Some(LIMIT as u64)), Some(LIMIT));
    }

    #[test]
    fn the_switches_default_to_off_and_a_sane_cap() {
        assert!(!capture_body_previews(), "capture is off until a panel asks for it");
        assert_eq!(body_capture_limit(), DEFAULT_BODY_CAPTURE_LIMIT);
    }
}

pub mod engine_event_emitter;
pub mod null_emitter;
#[cfg(feature = "timing")]
pub mod timing_emitter;
