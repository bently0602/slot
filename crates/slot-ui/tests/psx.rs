use slot_store::{Cart, Platform};
use slot_ui::{cart_box, psx_parts, Draw, PsxTextures, SlotChrome, TexId, OUT_H, OUT_W};

fn draws(seat: f32, yaw: f32) -> Vec<Draw> {
    let cart = Cart {
        platform: Platform::Psx,
        stem: "Adventure".into(),
        rom: "Adventure.chd".into(),
        label: None,
        code: String::new(),
        shell: None,
        title: "ADVENTURE".into(),
    };
    let mut out = Vec::new();
    SlotChrome {
        cart: &cart,
        face: Some(TexId::from_raw(1)),
        rest: (OUT_W - cart_box(Platform::Psx).0) as f32 / 2.0,
        scale: 1.0,
        seat,
        alert: None,
        dim: 0.0,
        screen: 0.0,
        game: false,
    }
    .draw_with_media(
        yaw,
        Some(PsxTextures {
            tray: TexId::from_raw(2),
            lid: TexId::from_raw(3),
            disc: TexId::from_raw(4),
        }),
        &mut out,
    );
    out
}

fn texture(draws: &[Draw], id: usize) -> Option<(f32, f32, f32, f32)> {
    draws.iter().find_map(|draw| match *draw {
        Draw::Tex {
            x, y, w, h, tex, ..
        }
        | Draw::Yawed {
            x,
            y,
            w,
            h,
            tex: Some(tex),
            ..
        } if tex == TexId::from_raw(id) => Some((x, y, w, h)),
        _ => None,
    })
}

#[test]
fn the_closed_cover_starts_at_the_shelf_pose_even_in_coverflow() {
    for yaw in [0.0, -0.7, 0.7] {
        let (x, _, w, h) = texture(&draws(0.0, yaw), 1).unwrap();
        assert!((x - (OUT_W as f32 - w) / 2.0).abs() < 0.001);
        assert_eq!((w, h), (281.0, 281.0));
    }
}

#[test]
fn the_case_opens_before_the_disc_lifts_and_only_the_disc_enters_the_slot() {
    let opened = draws(0.32, 0.0);
    let (tray_x, tray_y, tray_w, _) = texture(&opened, 2).unwrap();
    let (disc_x, disc_y, disc_w, _) = texture(&opened, 4).unwrap();
    assert!(disc_x > tray_x && disc_x + disc_w < tray_x + tray_w);
    assert!(disc_y > tray_y);
    assert!(
        texture(&opened, 3).unwrap().0 < tray_x,
        "lid did not hinge to the left"
    );
    let lifted = texture(&draws(0.54, 0.0), 4).unwrap();
    let lifted_draws = draws(0.54, 0.0);
    let lid_layer = lifted_draws
        .iter()
        .position(|d| {
            matches!(d,
        Draw::Yawed { tex: Some(tex), .. } if *tex == TexId::from_raw(3))
        })
        .unwrap();
    let disc_layer = lifted_draws
        .iter()
        .position(|d| {
            matches!(d,
        Draw::Tex { tex, .. } if *tex == TexId::from_raw(4))
        })
        .unwrap();
    assert!(
        disc_layer > lid_layer,
        "the lifted disc must pass in front of the lid"
    );
    assert!((lifted.0 + lifted.2 / 2.0 - OUT_W as f32 / 2.0).abs() < 0.001);
    assert!(lifted.1 < disc_y);
    let sliding = draws(0.85, 0.0);
    assert!(texture(&sliding, 4).unwrap().1 > lifted.1 + 150.0);
    for id in 1..=3 {
        assert!(texture(&sliding, id).is_none());
    }
    assert!(texture(&draws(0.99, 0.0), 4).unwrap().1 > OUT_H as f32 - 1.0);
    assert!(texture(&draws(1.0, 0.0), 4).is_none());
}

#[test]
fn the_optical_disc_has_a_transparent_outline_and_centre_hole() {
    let [tray, lid, disc] = psx_parts();
    assert_eq!((tray.w, tray.h), cart_box(Platform::Psx));
    assert_eq!(lid.rgba.len(), tray.rgba.len());
    let alpha = |x: u32, y: u32| disc.rgba[((y * disc.w + x) * 4 + 3) as usize];
    assert_eq!(alpha(0, 0), 0);
    assert_eq!(alpha(disc.w / 2, disc.h / 2), 0);
    assert_eq!(alpha(disc.w / 2, disc.h / 4), 255);
}
