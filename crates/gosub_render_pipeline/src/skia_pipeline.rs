

trait HasRenderPipeline<C: HasConfig> {
    type LayerList;
    type TileList;
    type Painter;
    type Rasterizer: Rasterable;
    type Compositor: Composable<Self>;
    type ComposeTarget: ComposeTarget;
    type TextureId;
}

trait ComposeTarget {}


trait Composable<P: HasRenderPipeline> {
    fn compose(target: &mut P::ComposeTarget, tiles: TileList<P>, visible_layers: Vec<bool>);
}





impl<P: HasRenderPipeline<ComposeTarget = skia::core::Canvas>> Composable<P> for SkiaComposer {
    fn compose(target: &mut P::ComposeTarget) {


    }
}


trait Rasterable<P: HasRenderPipeline> {
    fn rasterize(&self, tile: &Tile) -> Option<P::TextureId>;
}

SkiaComposer.compose(skai::Surface)
VelloComposer.compose(vello::Scene)



GenericComposer.compose(skia::Surface)
GenericComposer.compose(vello::Scene)

struct CairoComposer {
    type ComposeTarget = cairo::Context;

    fn compose(target: Self::ComposeTarget);
}

struct VelloComposer {
    type ComposeTarget = vello::Scene;

    fn compose(target: Self::ComposeTarget);
}








// struct SkiaPipeline {
//     document: Arc<Document>,
//     render_tree: RenderTreeImpl,
//     layouter: SkiaLayouterImpl,
//     layer_list: LayerListImpl,
//     tile_list: TileListImpl,
//     painter: PainterImpl,
//     rasterizer: SkiaRasterizerImpl,
//     compositor: SkiaCompositorImpl,
// }

impl HasRenderPipeline for SkiaPipeline {
    get_render_tree()
    get_layouter()
    get_layer_list()
    get_tile_list()
    get_painter()
    get_rasterizer()
    get_compositor()
}


DocumentImpl
Css3Impl
SkiaPipeline

SkiaPipelineImpl.get_compositor().compose(skia::core::Canvas, layer_ids: Vec<LayerId>);



trait HasRenderPipeLine:
    ???


    impl SkiaPipeline
    impl VelloPipeline
    impl CairoPipeline



trait Canvas {
    fn put_pixels(&self, data: Vec<u8>);
}


trait SkiaCanvas {
    fn put_texture(&self, texture: Texture);
}


default impl<T: Canvas> SkiaCanvas for T {
    fn put_texture(&self, texture: Texture) {
        self.put_pixels(texture.to_vec());
    }
}


impl SkiaCanvas for GPUCanvas {
    fn put_texture(&self, texture: Texture) {
        todo!()
    }
}


fn draw<T: SkiaCanvas>(data: Texture, canvas: &T) {
    canvas.put_texture(texture)

}