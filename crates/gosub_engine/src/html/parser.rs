use std::io;

use crate::html::{EngineDocument, RenderConfiguration};
use crate::net::types::{FetchResult, Priority, ResourceKind};
use crate::net::RequestDestination;
use cow_utils::CowUtils;
use gosub_html5::document::builder::DocumentBuilderImpl;
use gosub_html5::parser::Html5Parser;
use gosub_interface::css3::{CssOrigin, CssSystem};
use gosub_interface::document::Document as _;
use gosub_shared::byte_stream::{ByteStream, Encoding};
use gosub_shared::config::{Context, ParserConfig};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::sync::CancellationToken;
use url::Url;

/// A hint to the engine/IO layer that a subresource should be fetched.
#[derive(Debug, Clone)]
pub struct ResourceHint {
    /// Absolute URL of the resource to fetch.
    pub url: Url,
    /// The destination type (affects request headers, etc).
    pub dest: RequestDestination,
    /// The kind of resource (affects priority, etc).
    pub kind: ResourceKind,
    /// The `rel` attribute value if applicable.
    pub rel: Option<String>, // e.g. "stylesheet"
    /// The attribute we discovered this from.
    pub from_attr: &'static str, // e.g. "href" or "src"
    /// The referrer URL if applicable.
    pub referrer: Option<Url>,
    /// Whether this is a cross-origin request.
    pub cross_origin: bool,
    /// The integrity attribute value if applicable.
    pub integrity: Option<String>,
    /// Suggested fetch priority.
    pub priority: Priority,
}

/// Errors from buffering and parsing a main document stream.
#[derive(thiserror::Error, Debug)]
pub enum DocumentError {
    /// I/O error while reading the document stream.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// URL parsing error
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    /// Cancellation (navigation cancelled).
    #[error("Cancelled")]
    Cancelled,
}

/// Configuration for parsing a main document (see [`parse_main_document_stream`]).
#[derive(Debug, Clone)]
pub struct HtmlParseConfig {
    /// Max bytes to buffer from the stream; a larger document is truncated (with a warning).
    /// The engine reads this from the `net.document.max_bytes` setting.
    pub max_bytes: usize,
}

impl Default for HtmlParseConfig {
    fn default() -> Self {
        // Matches the `net.document.max_bytes` schema default.
        Self {
            max_bytes: 10 * 1024 * 1024,
        }
    }
}

/// Main entry point: buffer the HTML stream, parse it into a real DOM document,
/// and report discovered sub-resources.
///
/// - `base_url`: used to resolve relative URLs and as the document URL.
/// - `reader`: the response body stream (after the UA has chosen Render).
/// - `cancel`: cancellation token (tab/nav cancellation).
/// - `cfg`: buffer limit config.
/// - `on_discover`: callback invoked for each sub-resource hint found. It may return a receiver
///   for the resource body; stylesheets are awaited and attached to the document before it is
///   returned, so the caller gets a fully styled document. The parser itself performs no I/O.
pub async fn parse_main_document_stream<C, R, F>(
    base_url: Url,
    mut reader: R,
    cancel: CancellationToken,
    cfg: HtmlParseConfig,
    mut on_discover: F,
) -> Result<EngineDocument<C>, DocumentError>
where
    C: RenderConfiguration,
    R: AsyncRead + Unpin + Send + 'static,
    F: FnMut(ResourceHint) -> Option<tokio::sync::oneshot::Receiver<FetchResult>> + Send,
{
    // Buffer the full stream (up to cfg.max_bytes); bail on cancellation.
    let mut buf = Vec::with_capacity(32 * 1024);
    let mut tmp = [0u8; 16 * 1024];

    loop {
        if cancel.is_cancelled() {
            return Err(DocumentError::Cancelled);
        }

        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            break;
        }

        let remaining = cfg.max_bytes.saturating_sub(buf.len()).min(n);
        if remaining > 0 {
            buf.extend_from_slice(&tmp[..remaining]);
        }
        // If we hit the cap, we still drain the stream to EOF quickly
        // to avoid keeping the connection open unnecessarily.
        if buf.len() >= cfg.max_bytes {
            log::warn!(
                "Document {base_url} exceeds the {} byte limit (net.document.max_bytes); parsing truncated content",
                cfg.max_bytes
            );
            // Drain (non-blocking-ish) without growing memory
            // We don't strictly need to, but it's polite to the transport.
            let mut drain = [0u8; 16 * 1024];
            while reader.read(&mut drain).await? != 0 {
                if cancel.is_cancelled() {
                    return Err(DocumentError::Cancelled);
                }
            }
            break;
        }
    }

    // Use lossy UTF-8 only for the fast resource-discovery regex scan.
    let html_lossy = String::from_utf8_lossy(&buf);

    // Fire sub-resource callbacks using the fast regex-based scanner so that
    // image/CSS/script fetches are submitted before the full parse completes.
    //
    // Stylesheets are kept: the parser does no I/O, so the sheets it needs are the ones fetched
    // here, concurrently and on the connection the caller already has open. They're awaited after
    // the parse, below.
    let mut pending_sheets: Vec<(Url, tokio::sync::oneshot::Receiver<FetchResult>)> = Vec::new();
    for hint in discover_resources(&html_lossy, &base_url) {
        let sheet_url = matches!(hint.kind, ResourceKind::Stylesheet).then(|| hint.url.clone());
        let rx = on_discover(hint);
        if let (Some(url), Some(rx)) = (sheet_url, rx) {
            pending_sheets.push((url, rx));
        }
    }

    // Detect encoding from the raw bytes (BOM check + chardetng), then build a
    // properly-decoded stream.  We cannot call set_encoding() on an Unknown-
    // encoded stream because tell_bytes() returns buffer.len() when chars is
    // empty, which would advance the position to EOF.
    let encoding = {
        let mut tmp = ByteStream::new(Encoding::Unknown, None);
        tmp.read_from_bytes(&buf)?;
        tmp.detect_encoding()
    };
    let mut stream = ByteStream::new(encoding, None);
    stream.read_from_bytes(&buf)?;
    let mut doc = DocumentBuilderImpl::new_document::<C>(Some(base_url));
    let _ = Html5Parser::<C>::parse_document(&mut stream, &mut doc, None);

    // Author sheets first, then the UA sheet — the order the parser used to produce when it
    // fetched them itself. (Cascade origin decides precedence, but keep the order stable.)
    attach_external_stylesheets::<C>(&mut doc, pending_sheets, &cancel).await;

    let ua = <C::CssSystem as CssSystem>::load_default_useragent_stylesheet();
    doc.add_stylesheet(ua);

    // Optional user stylesheet, at `User` origin. This is the standard hook for an embedder to
    // impose its own constraints on every page — accessibility overrides, or (for the text-mode
    // backend) flattening line-height to one character cell. Sourced from an env var to match the
    // engine's other `GOSUB_*` hooks; use `!important` in the sheet to beat author declarations.
    if let Ok(css) = std::env::var("GOSUB_USER_STYLESHEET") {
        if !css.trim().is_empty() {
            attach_user_stylesheet::<C>(&mut doc, &css);
        }
    }

    Ok(doc)
}

/// Parse `css` at [`CssOrigin::User`] and attach it to `doc`. A parse failure is logged and
/// skipped rather than failing the navigation.
fn attach_user_stylesheet<C: RenderConfiguration>(doc: &mut EngineDocument<C>, css: &str) {
    let config = ParserConfig {
        context: Context::Stylesheet,
        location: Default::default(),
        source: Some("user-stylesheet".to_string()),
        ignore_errors: true,
        match_values: false,
    };
    match <C::CssSystem as CssSystem>::parse_str(css, config, CssOrigin::User, "user-stylesheet") {
        Ok(sheet) => doc.add_stylesheet(sheet),
        Err(err) => log::warn!("Error parsing user stylesheet: {err}"),
    }
}

/// Await the stylesheet bodies discovered before the parse, and attach them to `doc`.
///
/// They were requested up-front, so by now they have been downloading for the length of the parse
/// and are usually already there. Sheets are awaited in document order; a sheet that fails, 404s,
/// or arrives as a stream is skipped with a warning rather than failing the navigation — a page
/// with a broken stylesheet still renders.
async fn attach_external_stylesheets<C: RenderConfiguration>(
    doc: &mut EngineDocument<C>,
    pending: Vec<(Url, tokio::sync::oneshot::Receiver<FetchResult>)>,
    cancel: &CancellationToken,
) {
    for (url, rx) in pending {
        if cancel.is_cancelled() {
            return;
        }

        let body = tokio::select! {
            res = rx => res,
            _ = cancel.cancelled() => return,
        };

        let body = match body {
            Ok(FetchResult::Buffered { meta, body }) if meta.status == 200 => body,
            Ok(FetchResult::Buffered { meta, .. }) => {
                log::warn!("Stylesheet {url} returned status {}; skipping", meta.status);
                continue;
            }
            Ok(FetchResult::Error(err)) => {
                log::warn!("Could not load stylesheet {url}: {err}");
                continue;
            }
            // Stylesheets are requested non-streaming, so this should not happen.
            Ok(FetchResult::Stream { .. }) => {
                log::warn!("Stylesheet {url} arrived as a stream; skipping");
                continue;
            }
            Err(_) => {
                log::warn!("Stylesheet {url} fetch was dropped before completing");
                continue;
            }
        };

        let css = match std::str::from_utf8(&body) {
            Ok(css) => css,
            Err(err) => {
                log::warn!("Stylesheet {url} is not valid UTF-8: {err}");
                continue;
            }
        };

        let config = ParserConfig {
            context: Context::Stylesheet,
            location: Default::default(),
            source: Some(url.to_string()),
            ignore_errors: true,
            match_values: false,
        };

        match <C::CssSystem as CssSystem>::parse_str(css, config, CssOrigin::Author, url.as_str()) {
            Ok(sheet) => doc.add_stylesheet(sheet),
            Err(err) => log::warn!("Error parsing stylesheet {url}: {err}"),
        }
    }
}

// ======== Forgiving resource discovery (regex-based) ========
fn unquote(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 && ((b[0] == b'"' && b[b.len() - 1] == b'"') || (b[0] == b'\'' && b[b.len() - 1] == b'\'')) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Compile a literal regex pattern.
fn re(pattern: &str) -> Regex {
    #[allow(clippy::unwrap_used)] // PANIC-SAFE: all callers pass literal patterns, exercised by tests
    Regex::new(pattern).unwrap()
}

static RE_LINK_STYLESHEET: Lazy<Regex> = Lazy::new(|| {
    // allow "..." or '...' or unquoted; capture into the *same* group `href`
    re(
        r#"(?is)<\s*link\b[^>]*\brel\s*=\s*(?:"stylesheet"|'stylesheet')[^>]*\bhref\s*=\s*(?P<href>"[^"]*"|'[^']*'|[^\s>]+)[^>]*>"#,
    )
});

static RE_SCRIPT_SRC: Lazy<Regex> =
    Lazy::new(|| re(r#"(?is)<\s*script\b[^>]*\bsrc\s*=\s*(?P<src>"[^"]*"|'[^']*'|[^\s>]+)[^>]*>"#));

static RE_ASYNC_ATTR: Lazy<Regex> = Lazy::new(|| re(r#"\basync\b"#));

static RE_DEFER_ATTR: Lazy<Regex> = Lazy::new(|| re(r#"\bdefer\b"#));

static RE_IMG_SRC: Lazy<Regex> =
    Lazy::new(|| re(r#"(?is)<\s*img\b[^>]*\bsrc\s*=\s*(?P<src>"[^"]*"|'[^']*'|[^\s>]+)[^>]*>"#));

fn discover_resources(html: &str, base: &Url) -> Vec<ResourceHint> {
    let mut out = Vec::new();

    // Stylesheets
    for cap in RE_LINK_STYLESHEET.captures_iter(html) {
        let Some(m) = cap.name("href") else {
            continue;
        };
        let Ok(u) = resolve(base, unquote(m.as_str())) else {
            continue;
        };
        out.push(ResourceHint {
            url: u,
            dest: RequestDestination::Document,
            referrer: None,
            cross_origin: false,
            integrity: None,
            kind: ResourceKind::Stylesheet,
            rel: Some("stylesheet".to_string()),
            from_attr: "href",
            priority: Priority::High,
        });
    }

    // Scripts
    for cap in RE_SCRIPT_SRC.captures_iter(html) {
        let tag = cap.get(0).map_or("", |m| m.as_str());
        let tag_lower = tag.cow_to_ascii_lowercase();
        // A script is blocking unless it has async or defer attributes
        let blocking = !RE_ASYNC_ATTR.is_match(tag_lower.as_ref()) && !RE_DEFER_ATTR.is_match(tag_lower.as_ref());
        let Some(m) = cap.name("src") else {
            continue;
        };
        let Ok(u) = resolve(base, unquote(m.as_str())) else {
            continue;
        };
        out.push(ResourceHint {
            url: u,
            kind: ResourceKind::Script { blocking },
            rel: None,
            from_attr: "src",
            dest: RequestDestination::Script,
            referrer: None,
            cross_origin: false,
            integrity: None,
            priority: Priority::Normal,
        });
    }

    // Images
    for cap in RE_IMG_SRC.captures_iter(html) {
        let Some(m) = cap.name("src") else {
            continue;
        };
        let Ok(u) = resolve(base, unquote(m.as_str())) else {
            continue;
        };
        out.push(ResourceHint {
            url: u,
            kind: ResourceKind::Image,
            rel: None,
            from_attr: "src",
            dest: RequestDestination::Image,
            referrer: None,
            cross_origin: false,
            integrity: None,
            priority: Priority::Low,
        });
    }

    out
}

fn resolve(base: &Url, candidate: &str) -> Result<Url, url::ParseError> {
    // Tolerate whitespace, no-op fragments, etc.
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return Err(url::ParseError::EmptyHost);
    }
    base.join(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::DefaultRenderConfig;
    use bytes::Bytes;
    use futures::stream;
    use tokio_util::io::StreamReader;

    fn reader_from_str(s: &str) -> impl AsyncRead + Unpin + Send + 'static {
        // One-chunk stream -> AsyncRead
        let it = stream::iter(vec![Ok::<Bytes, io::Error>(Bytes::from(s.to_owned()))]);
        StreamReader::new(it)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn parses_title_and_discovers_resources() {
        let html = r#"
            <html>
              <head>
                <title> Hello World </title>
                <link rel="stylesheet" href="/style.css">
              </head>
              <body>
                <script src="app.js"></script>
                <img src="images/logo.png">
              </body>
            </html>
        "#;

        let base = Url::parse("https://example.com/path/index.html").unwrap();
        let cancel = CancellationToken::new();
        let mut hints = Vec::new();

        parse_main_document_stream::<DefaultRenderConfig, _, _>(
            base.clone(),
            reader_from_str(html),
            cancel,
            HtmlParseConfig::default(),
            |h| {
                hints.push(h);
                None
            },
        )
        .await
        .unwrap();

        // Ensure we discovered 3 resources with resolved URLs
        assert_eq!(hints.len(), 3);
        assert!(hints
            .iter()
            .any(|h| h.kind == ResourceKind::Stylesheet && h.url.as_str() == "https://example.com/style.css"));
        assert!(hints.iter().any(|h| h.kind == ResourceKind::Script { blocking: true }
            && h.url.as_str() == "https://example.com/path/app.js"));
        assert!(hints
            .iter()
            .any(|h| h.kind == ResourceKind::Image && h.url.as_str() == "https://example.com/path/images/logo.png"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn honors_cancellation() {
        let base = Url::parse("https://e.test/").unwrap();

        // Make a stream that hangs so we can cancel before read completes.
        use futures::stream::pending;
        let pending_stream = pending::<Result<Bytes, io::Error>>();
        let reader = StreamReader::new(pending_stream);

        let cancel = CancellationToken::new();
        cancel.cancel(); // cancel immediately

        let res = parse_main_document_stream::<DefaultRenderConfig, _, _>(
            base,
            reader,
            cancel,
            HtmlParseConfig::default(),
            |_h| None,
        )
        .await;

        match res {
            Err(DocumentError::Cancelled) => {}
            other => panic!("expected Cancelled, got {:?}", other),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn truncates_at_max_bytes() {
        let base = Url::parse("https://e.test/").unwrap();
        let big = "A".repeat(150_000); // 150 KiB
        let cfg = HtmlParseConfig { max_bytes: 64 * 1024 }; // 64 KiB

        // Just verify truncated input still produces a valid document (no panic).
        parse_main_document_stream::<DefaultRenderConfig, _, _>(
            base,
            reader_from_str(&big),
            CancellationToken::new(),
            cfg,
            |_h| None,
        )
        .await
        .unwrap();
    }
}
