use crate::CairoBackend;
use gosub_interface::render_backend::RenderText;

use crate::elements::brush::GsBrush;
use crate::elements::color::GsColor;
use kurbo::Stroke;
use log::info;
use pango::Layout;
use gosub_renderer::font::Font as GsRenderFont;

#[allow(unused)]
#[derive(Clone, Debug)]
pub struct GsText {
    /// Actual utf-8 text
    text: String,
    /// Font in which we need to display the text
    font: GsRenderFont,
    /// Position of the text (top-left corner)
    tl_pos: [f64; 2],
}

impl GsText {
    pub(crate) fn render(obj: &RenderText<CairoBackend>, cr: &cairo::Context) {
        info!(target: "cairo", "GsText::render");

        let pango_ctx = pangocairo::functions::create_context(cr);
        let layout = Layout::new(&pango_ctx);

        let font_desc = &obj.font.get_font_description();
        layout.set_font_description(Some(&font_desc));
        layout.set_text(&obj.text);

        // Setup brush for rendering text
        GsBrush::render(&obj.brush, cr);

        cr.move_to(obj.rect.x.into(), obj.rect.y.into());
        cr.set_source_rgb(0.0, 0.0, 1.0);
        pangocairo::functions::show_layout(cr, &layout);
    }
}
