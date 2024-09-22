use std::fs;

use anyhow::bail;
use gosub_net::http::fetcher::Fetcher;
use gosub_net::http::ureq;
use gosub_render_backend::geo::SizeU32;
use gosub_render_backend::layout::Layouter;
use gosub_render_backend::RenderBackend;
use gosub_rendering::position::PositionTree;
use gosub_rendering::render_tree::{generate_render_tree, RenderTree};
use gosub_shared::byte_stream::{ByteStream, Encoding};
use gosub_shared::document::DocumentHandle;
use gosub_shared::node::NodeId;
use gosub_shared::traits::css3::CssSystem;
use gosub_shared::traits::document::{Document, DocumentBuilder};
use gosub_shared::traits::html5::Html5Parser;
use url::Url;

pub struct TreeDrawer<B: RenderBackend, L: Layouter, D: Document<C>, C: CssSystem> {
    pub(crate) fetcher: Fetcher,
    pub(crate) tree: RenderTree<L, D, C>,
    pub(crate) layouter: L,
    pub(crate) size: Option<SizeU32>,
    pub(crate) position: PositionTree,
    pub(crate) last_hover: Option<NodeId>,
    pub(crate) debug: bool,
    pub(crate) dirty: bool,
    pub(crate) debugger_scene: Option<B::Scene>,
    pub(crate) tree_scene: Option<B::Scene>,
    pub(crate) selected_element: Option<NodeId>,
    pub(crate) scene_transform: Option<B::Transform>,
}

impl<B: RenderBackend, L: Layouter, D: Document<C>, C: CssSystem> TreeDrawer<B, L, D, C> {
    pub fn new(tree: RenderTree<L, D, C>, layouter: L, url: Url, debug: bool) -> Self {
        Self {
            tree,
            layouter,
            size: None,
            position: PositionTree::default(),
            last_hover: None,
            debug,
            debugger_scene: None,
            dirty: false,
            tree_scene: None,
            selected_element: None,
            scene_transform: None,
            fetcher: Fetcher::new(url),
        }
    }
}

// pub struct RenderTreeNode<L: Layouter> {
//     pub parent: Option<NodeId>,
//     pub children: Vec<NodeId>,
//     pub layout: i32, //TODO
//     pub name: String,
//     pub properties: CssProperties,
//     pub namespace: Option<String>,
//     pub data: RenderNodeData<L>,
// }

pub(crate) fn load_html_rendertree<L: Layouter, P: Html5Parser<C>, C: CssSystem>(
    url: Url,
) -> gosub_shared::types::Result<RenderTree<L, P::Document, C>> {
    let html = if url.scheme() == "http" || url.scheme() == "https" {
        // Fetch the html from the url
        let response = ureq::get(url.as_ref()).call()?;
        if response.status() != 200 {
            bail!(format!("Could not get url. Status code {}", response.status()));
        }
        response.into_string()?
    } else if url.scheme() == "file" {
        fs::read_to_string(url.as_str().trim_start_matches("file://"))?
    } else {
        bail!("Unsupported url scheme: {}", url.scheme());
    };

    let mut stream = ByteStream::new(Encoding::UTF8, None);
    stream.read_from_str(&html, Some(Encoding::UTF8));
    stream.close();

    let mut doc_handle = <P::Document as Document<C>>::Builder::new_document(Some(url));
    let parse_errors = P::parse(&mut stream, DocumentHandle::clone(&doc_handle), None)?;

    for error in parse_errors {
        eprintln!("Parse error: {:?}", error);
    }

    _ = doc_handle.get_mut();

    // doc.add_stylesheet(C::load_default_useragent_stylesheet()?);

    // drop(doc);

    generate_render_tree(DocumentHandle::clone(&doc_handle))
}
