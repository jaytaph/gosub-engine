use crate::VelloBackend;
use gosub_interface::layout::{TextLayout};
use gosub_interface::render_backend::{RenderText};
use gosub_shared::geo::FP;
use vello::kurbo::{Affine, Line, Stroke};
use vello::peniko::{Brush, Color, Fill, Font, StyleRef};
use vello::skrifa::instance::NormalizedCoord;
use vello::Scene;
use vello_encoding::Glyph;

#[derive(Clone)]
pub struct Text {
    text: String,
    font: Font,
    coords: Vec<NormalizedCoord>,
}

impl Text {
    pub(crate) fn show(scene: &mut Scene, render: &RenderText<VelloBackend>) {
        let brush = &render.brush.0;
        let style: StyleRef = Fill::NonZero.into();

        let transform = render.transform.map(|t| t.0).unwrap_or(Affine::IDENTITY);
        let brush_transform = render.brush_transform.map(|t| t.0);

        let x = render.rect.0.x0;
        let y = render.rect.0.y0;

        let transform = transform.with_translation((x, y).into());

        scene
            .draw_glyphs(&render.text.font)
            .font_size(render.text.fs)
            .transform(transform)
            .glyph_transform(brush_transform)
            .normalized_coords(&render.text.coords)
            .brush(brush)
            .draw(style, render.text.glyphs.iter().copied());

        {
            let decoration = &render.text.decoration;

            let stroke = Stroke::new(decoration.width as f64);

            let c = decoration.color;

            let brush = Brush::Solid(Color::rgba(c.0 as f64, c.1 as f64, c.2 as f64, c.3 as f64));

            let offset = decoration.x_offset as f64;

            if decoration.underline {
                let y = y + decoration.underline_offset as f64;

                let line = Line::new((x + offset, y), (x + render.rect.0.width(), y));

                scene.stroke(&stroke, Affine::IDENTITY, &brush, None, &line);
            }

            if decoration.overline {
                let y = y - render.rect.0.height();

                let line = Line::new((x + offset, y), (x + render.rect.0.width(), y));

                scene.stroke(&stroke, Affine::IDENTITY, &brush, None, &line);
            }

            if decoration.line_through {
                let y = y - render.rect.0.height() / 2.0;

                let line = Line::new((x + offset, y), (x + render.rect.0.width(), y));

                scene.stroke(&stroke, Affine::IDENTITY, &brush, None, &line);
            }
        }
    }
}
