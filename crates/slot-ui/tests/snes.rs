use slot_store::{Cart, Platform};
use slot_ui::{cart_box, cart_face, cart_face_with, label_size, seated_box};

#[test]
fn snes_cartridge_has_its_own_shape_and_keeps_art_inside_the_label() {
    let cart = Cart {
        platform: Platform::Snes,
        stem: "Slot SNES Test".into(),
        rom: "Test.sfc".into(),
        label: None,
        title: "Slot SNES Test".into(),
        code: String::new(),
        shell: None,
    };
    let face = cart_face(&cart);
    assert_eq!((face.w, face.h), cart_box(Platform::Snes));
    assert_eq!(seated_box(Platform::Snes), (240, 150));
    assert_eq!(face.rgba.len(), (face.w * face.h * 4) as usize);
    assert!(face.rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
    if let Some(path) = std::env::var_os("SLOT_TEST_SNES_CART_PNG") {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, face.w, face.h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&face.rgba)
            .unwrap();
    }
    let (w, h) = label_size(&cart);
    let face = cart_face_with(&cart, Some([255, 0, 0, 255].repeat((w * h) as usize)));
    let red = face
        .rgba
        .chunks_exact(4)
        .filter(|pixel| *pixel == [255, 0, 0, 255])
        .count();
    assert_eq!(red, (w * h) as usize);
}
