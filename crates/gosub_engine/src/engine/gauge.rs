//! `<meter>` and `<progress>`: the numbers behind the bar.
//!
//! Both elements carry raw attributes that only mean something after a chain of defaults and
//! clamps, and the order of that chain matters (`high` is clamped against the already-clamped
//! `low`, not against the raw attribute). Painting one of these will want exactly these
//! numbers, so the algorithm lives here rather than in whatever asks for it.

use crate::engine::edit::parse_number;
use crate::html::{DomConfiguration, EngineDocument};
use gosub_interface::document::Document as _;
use gosub_shared::node::NodeId;

/// A `<meter>`'s resolved gauge values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meter {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub low: f64,
    pub high: f64,
    pub optimum: f64,
}

/// A `<progress>`'s resolved values. `position` is `-1.0` when the bar is indeterminate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub value: f64,
    pub max: f64,
    pub position: f64,
}

fn number<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, name: &str) -> Option<f64> {
    parse_number(doc.attribute(id, name)?.trim())
}

/// Resolve a `<meter>`'s attributes into the six numbers it displays.
pub fn meter<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Meter {
    let min = number(doc, id, "min").unwrap_or(0.0);
    // `max` defaults to 1, but a maximum below the minimum collapses onto it - so a meter
    // with `min="12.1"` and no `max` has both ends at 12.1.
    let max = number(doc, id, "max").unwrap_or(1.0).max(min);
    let value = number(doc, id, "value").unwrap_or(0.0).clamp(min, max);
    let low = number(doc, id, "low").unwrap_or(min).clamp(min, max);
    // Clamped against `low` after `low` itself settled, not against the raw attribute.
    let high = number(doc, id, "high").unwrap_or(max).clamp(low, max);
    let optimum = number(doc, id, "optimum").unwrap_or((min + max) / 2.0).clamp(min, max);
    Meter {
        value,
        min,
        max,
        low,
        high,
        optimum,
    }
}

/// Resolve a `<progress>`'s attributes. A missing or unparseable `value` leaves the bar
/// indeterminate, which is what `position == -1.0` means.
pub fn progress<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Progress {
    let max = number(doc, id, "max").filter(|m| *m > 0.0).unwrap_or(1.0);
    match number(doc, id, "value") {
        Some(value) => {
            let value = value.clamp(0.0, max);
            Progress {
                value,
                max,
                position: value / max,
            }
        }
        None => Progress {
            value: 0.0,
            max,
            position: -1.0,
        },
    }
}
