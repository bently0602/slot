use slot_retro::{GBA_H, GBA_W};

pub fn png(xrgb8888: &[u8]) -> Option<Vec<u8>> {
    let n = (GBA_W * GBA_H) as usize;
    if xrgb8888.len() < n * 4 {
        return None;
    }
    let mut rgb = Vec::with_capacity(n * 3);
    if xrgb8888.len() == 640 * 480 * 4 {
        for y in 0..GBA_H as usize {
            for x in 0..GBA_W as usize {
                if !(13..226).contains(&x) {
                    rgb.extend_from_slice(&[0; 3]);
                    continue;
                }
                let at = (y * 480 / GBA_H as usize * 640 + (x - 13) * 640 / 213) * 4;
                rgb.extend_from_slice(&[xrgb8888[at + 2], xrgb8888[at + 1], xrgb8888[at]]);
            }
        }
    } else {
        for px in xrgb8888[..n * 4].chunks_exact(4) {
            rgb.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }

    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, GBA_W, GBA_H);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().ok()?;
    writer.write_image_data(&rgb).ok()?;
    writer.finish().ok()?;
    Some(out)
}
