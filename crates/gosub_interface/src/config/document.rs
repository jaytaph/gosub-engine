use crate::config::css_system::HasCssSystem;
use crate::document::{Document, DocumentBuilder, DocumentFragment};
use crate::html5::Html5Parser;
use crate::node::{CommentDataType, DocTypeDataType, DocumentDataType, ElementDataType, Node, TextDataType};
use std::fmt::Debug;

/// Minimal trait for types that have an associated document fragment type.
/// Used to break the recursive type cycle in ElementData without requiring full HasDocument.
pub trait HasDocumentFragment: Sized {
    type DocumentFragment: Clone + Debug + PartialEq + 'static;
}

/// Blanket impl: all HasDocument types also implement HasDocumentFragment.
impl<C: HasDocument> HasDocumentFragment for C {
    type DocumentFragment = C::DocumentFragment;
}

pub trait HasDocument:
    Sized
    + Clone
    + Debug
    + PartialEq
    + HasCssSystem
    + 'static
    + HasDocumentExt<
        Self,
        Node = <Self::Document as Document<Self>>::Node,
        DocumentData = <<Self::Document as Document<Self>>::Node as Node<Self>>::DocumentData,
        DocTypeData = <<Self::Document as Document<Self>>::Node as Node<Self>>::DocTypeData,
        TextData = <<Self::Document as Document<Self>>::Node as Node<Self>>::TextData,
        CommentData = <<Self::Document as Document<Self>>::Node as Node<Self>>::CommentData,
        ElementData = <<Self::Document as Document<Self>>::Node as Node<Self>>::ElementData,
    >
{
    type Document: Document<Self>;
    type DocumentFragment: DocumentFragment<Self>;

    type DocumentBuilder: DocumentBuilder<Self>;
}

pub trait HasHtmlParser: HasDocument {
    type HtmlParser: Html5Parser<Self, Stream = Self::HtmlStream>;
    type HtmlStream: 'static;
}

pub trait HasDocumentExt<C: HasDocument> {
    type Node: Node<C>;
    type DocumentData: DocumentDataType;
    type DocTypeData: DocTypeDataType;
    type TextData: TextDataType;
    type CommentData: CommentDataType;
    type ElementData: ElementDataType<C>;
}

impl<C: HasDocument> HasDocumentExt<C> for C {
    type Node = <C::Document as Document<Self>>::Node;
    type DocumentData = <<C::Document as Document<Self>>::Node as Node<Self>>::DocumentData;
    type DocTypeData = <<C::Document as Document<Self>>::Node as Node<Self>>::DocTypeData;
    type TextData = <<C::Document as Document<Self>>::Node as Node<Self>>::TextData;
    type CommentData = <<C::Document as Document<Self>>::Node as Node<Self>>::CommentData;
    type ElementData = <<C::Document as Document<Self>>::Node as Node<Self>>::ElementData;
}
