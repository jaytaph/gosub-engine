//! `DOMException`.
//!
//! Tests do not just check that something threw - `assert_throws_dom` checks the thrown
//! value is a `DOMException` with the right `name` and legacy `code`, so a plain `Error`
//! fails even when the engine behaved correctly.

use rquickjs::class::Trace;
use rquickjs::prelude::Opt;
use rquickjs::{Class, Ctx, JsLifetime};

/// The legacy `code` numbers. Names outside this table report 0, as the spec says.
const LEGACY_CODES: [(&str, u32); 20] = [
    ("IndexSizeError", 1),
    ("HierarchyRequestError", 3),
    ("WrongDocumentError", 4),
    ("InvalidCharacterError", 5),
    ("NoModificationAllowedError", 7),
    ("NotFoundError", 8),
    ("NotSupportedError", 9),
    ("InUseAttributeError", 10),
    ("InvalidStateError", 11),
    ("SyntaxError", 12),
    ("InvalidModificationError", 13),
    ("NamespaceError", 14),
    ("InvalidAccessError", 15),
    ("TypeMismatchError", 17),
    ("SecurityError", 18),
    ("NetworkError", 19),
    ("AbortError", 20),
    ("URLMismatchError", 21),
    ("QuotaExceededError", 22),
    ("TimeoutError", 23),
];

#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename = "DOMException")]
pub struct DomException {
    #[qjs(skip_trace)]
    name: String,
    #[qjs(skip_trace)]
    message: String,
}

#[rquickjs::methods(rename_all = "camelCase")]
impl DomException {
    #[qjs(constructor)]
    pub fn new(message: Opt<String>, name: Opt<String>) -> Self {
        Self {
            name: name.0.unwrap_or_else(|| "Error".to_string()),
            message: message.0.unwrap_or_default(),
        }
    }

    #[qjs(get)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[qjs(get)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    #[qjs(get)]
    pub fn code(&self) -> u32 {
        LEGACY_CODES
            .iter()
            .find(|(name, _)| *name == self.name)
            .map_or(0, |(_, code)| *code)
    }

    #[qjs(rename = "toString")]
    pub fn to_string_js(&self) -> String {
        format!("{}: {}", self.name, self.message)
    }
}

/// Throw a `DOMException` with `name`, so `assert_throws_dom` recognises it.
pub fn throw(ctx: &Ctx<'_>, name: &str, message: &str) -> rquickjs::Error {
    let exception = DomException {
        name: name.to_string(),
        message: message.to_string(),
    };
    match Class::instance(ctx.clone(), exception) {
        Ok(instance) => ctx.throw(instance.into_value()),
        Err(e) => e,
    }
}

pub fn install(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    Class::<DomException>::define(&ctx.globals())
}
