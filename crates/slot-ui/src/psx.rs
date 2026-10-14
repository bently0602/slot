use slot_gfx::{Draw, TexId, OUT_H, OUT_W};

use crate::cart::{cart_box, CartFace};
use crate::shelf::foot_y;
use crate::slot_chrome::ease;
use slot_store::Platform;

#[derive(Copy, Clone)]
pub struct PsxTextures {
    pub tray: TexId,
    pub lid: TexId,
    pub disc: TexId,
}

/// Reusable jewel-case parts; the selected game's shelf cover is the outer lid.
pub fn psx_parts() -> [CartFace; 3] {
    let (w, h) = cart_box(Platform::Psx);
    let mut tray = vec![0; (w * h * 4) as usize];
    let mut lid = tray.clone();
    let mut disc = tray.clone();
    for y in 0..h {
        for x in 0..w {
            let at = ((y * w + x) * 4) as usize;
            let dx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / h as f32 * 2.0 - 1.0;
            let radius = dx.hypot(dy);
            let border = x < 5 || y < 5 || x >= w - 5 || y >= h - 5;
            let c = if border {
                [143, 151, 160, 255]
            } else if x < 15 {
                [57, 61, 69, 255]
            } else if radius < 0.13 {
                [82, 87, 96, 255]
            } else if (0.91..0.96).contains(&radius) {
                [70, 75, 83, 255]
            } else {
                [29, 32, 38, 255]
            };
            tray[at..at + 4].copy_from_slice(&c);
            let c = if border {
                [182, 190, 198, 255]
            } else if x < 13 || y < 12 || y >= h - 12 {
                [104, 112, 123, 230]
            } else {
                [177, 188, 199, 95]
            };
            lid[at..at + 4].copy_from_slice(&c);
            if radius < 0.085 || radius > 1.0 {
                continue;
            }
            let angle = dy.atan2(dx);
            let shine = (angle * 2.0 + radius * 4.0).cos();
            let base = 192.0 + 36.0 * shine;
            let c = if radius < 0.18 {
                [100, 108, 119]
            } else if radius < 0.23 || radius > 0.975 {
                [229, 233, 239]
            } else {
                [0, 1, 2].map(|channel| {
                    (base + 19.0 * (angle * 3.0 + channel as f32 * 2.1).sin()).clamp(0.0, 255.0)
                        as u8
                })
            };
            let coverage = ((1.0 - radius) * w as f32 / 2.0).clamp(0.0, 1.0);
            disc[at..at + 3].copy_from_slice(&c);
            disc[at + 3] = (coverage * 255.0) as u8;
        }
    }
    [tray, lid, disc].map(|rgba| CartFace { rgba, w, h })
}

fn phase(seat: f32, from: f32, to: f32) -> f32 {
    ease(((seat - from) / (to - from)).clamp(0.0, 1.0))
}

fn panel(tex: Option<TexId>, x: f32, y: f32, w: f32, h: f32, alpha: f32) -> Draw {
    match tex {
        Some(tex) => Draw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha,
        },
        None => Draw::Rect {
            x,
            y,
            w,
            h,
            colour: [0.15, 0.17, 0.20, alpha],
        },
    }
}

/// The same progress runs backwards on eject, including a launch interrupted mid-open.
/// Returns the disc bounds for the launch-failure alert.
pub(crate) fn draw_psx(
    face: Option<TexId>,
    parts: Option<PsxTextures>,
    rest: f32,
    scale: f32,
    seat: f32,
    yaw: f32,
    out: &mut Vec<Draw>,
) -> (f32, f32, f32, f32) {
    let size = cart_box(Platform::Psx).0 as f32;
    let open = phase(seat, 0.0, 0.32);
    let lift = phase(seat, 0.32, 0.54);
    let slide = phase(seat, 0.54, 1.0);
    let fade = 1.0 - phase(seat, 0.56, 0.80);
    let w = size * scale + (218.0 - size * scale) * open;
    let x = rest + (OUT_W as f32 / 2.0 + 65.0 - rest) * open;
    let y0 = foot_y(size) - size * scale;
    let y = y0 + (85.0 - y0) * open;
    let disc_w = w * 0.89 + (240.0 - w * 0.89) * lift;
    let disc_x = x + w * 0.055 + (OUT_W as f32 / 2.0 - disc_w / 2.0 - x - w * 0.055) * lift;
    let disc_y = y + w * 0.055 - 25.0 * lift;
    let disc_y = disc_y + (OUT_H as f32 - disc_y) * slide;

    if fade > 0.0 {
        out.push(panel(parts.map(|p| p.tray), x, y, w, w, fade).yawed(yaw * (1.0 - open)));
    }
    let lid = if fade > 0.0 {
        let turn = open * 2.65;
        let lid_yaw = yaw * (1.0 - open) - turn;
        // Anchor the left edge at the hinge, accounting for the sprite shader's
        // perspective around its centre (eye distance: 600 px).
        let initial_yaw = yaw * (1.0 - open);
        let hinge_x =
            x + w / 2.0 - w / 2.0 * initial_yaw.cos() / (1.0 + initial_yaw.sin() * w / 1200.0);
        let lid_x =
            hinge_x - w / 2.0 + w / 2.0 * lid_yaw.cos() / (1.0 + lid_yaw.sin() * w / 1200.0);
        let lid = if turn < std::f32::consts::FRAC_PI_2 {
            face
        } else {
            parts.map(|p| p.lid)
        };
        Some(panel(lid, lid_x, y, w, w, fade).yawed(lid_yaw))
    } else {
        None
    };
    // Once lifted free, the disc passes in front of the open lid.
    if lift > 0.0 {
        out.extend(lid);
    }
    if seat < 1.0 {
        if let Some(parts) = parts {
            out.push(
                panel(Some(parts.disc), disc_x, disc_y, disc_w, disc_w, 1.0)
                    .yawed(yaw * (1.0 - open)),
            );
        } else {
            // Keep a circular disc visible before the shared textures are uploaded.
            for row in 0..64 {
                let v = (row as f32 + 0.5) / 32.0 - 1.0;
                let half = (1.0 - v * v).sqrt() * disc_w / 2.0;
                out.push(Draw::Rect {
                    x: disc_x + disc_w / 2.0 - half,
                    y: disc_y + row as f32 * disc_w / 64.0,
                    w: half * 2.0,
                    h: disc_w / 64.0,
                    colour: [0.78, 0.81, 0.85, 1.0],
                });
            }
        }
    }
    if lift == 0.0 {
        out.extend(lid);
    }
    (disc_x, disc_y, disc_w, disc_w)
}
