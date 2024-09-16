use crate::document::DocumentHandle;
use crate::traits::css3::CssSystem;
use crate::traits::document::Document;

use crate::types::Result;

pub trait Html5Parser<C: CssSystem> {

    type Document: Document<C>;

    fn parse(&self, data: String) -> Result<DocumentHandle<Self::Document, C>>;
}
