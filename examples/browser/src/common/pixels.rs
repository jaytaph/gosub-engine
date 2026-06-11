/// Convert premultiplied BGRA (Cairo ARGB32 / Skia N32, LE bytes: B G R A) to
/// RGBA8 with straight alpha, as egui textures expect.
pub fn bgra_premul_to_rgba8(pixels: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        for col in 0..width {
            let off = row * stride + col * 4;
            let b = pixels[off];
            let g = pixels[off + 1];
            let r = pixels[off + 2];
            let a = pixels[off + 3];
            if a == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else if a == 255 {
                out.extend_from_slice(&[r, g, b, 255]);
            } else {
                let af = a as f32 / 255.0;
                out.push((r as f32 / af).min(255.0) as u8);
                out.push((g as f32 / af).min(255.0) as u8);
                out.push((b as f32 / af).min(255.0) as u8);
                out.push(a);
            }
        }
    }
    out
}
