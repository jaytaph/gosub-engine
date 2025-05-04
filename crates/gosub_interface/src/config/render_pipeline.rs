use std::collections::HashMap;
use std::sync::Arc;
use crate::config::HasLayouter;
use crate::font::HasFontManager;
use crate::render_tree::RenderTree;

pub trait HasRenderPipeline: HasDocument + CanRenderTree + CanLayouter + CanLayerList + CanTileList + CanPainter + CanRasterize + CanComposite + HasFontManager {
}


struct DirectPipelineImpl;


impl HasRenderPipeline for DirectPipelineImpl {


}



struct MultiStepPipeline;




trait HasPainter<C: HasDocument> {}
trait HasRasterizer {}



impl HasPainter for MultiStepPipeline {


}






/** This trait should implement a complete pipeline:

1. Document
2. Render tree
3. Layouter
4. Layer list
5. Tile list
6. Painter
7. Rasterizer
8. Composite

So we have something like:

SkiaPipeline:
    generic RenderTree
    skiaFont Layouter
    generic layer lister
    generic tile lister
    generic painter
    skia rasterizer
    skia compositor
*/


pub trait CanRenderTree {
    type RenderTree: RenderTree<Self>;

    fn new(doc: Arc<Document>) -> &Self::RenderTree;

    // These things should be on rendertree, not the hasRenderTree trait?
    // fn parse(&mut self) -> Result<(), String>;
    // fn get_node_by_id(&self, node_id: RenderNodeId) -> Option<&RenderNode>;
    // fn get_dom_node_by_render_id(&self, render_node_id: RenderNodeId) -> Option<&Node>;
}

pub trait HasLayouter {
    type Layouter: Layouter<Self>;
    fn get_layouter(&self) -> &Self::Layouter;
}