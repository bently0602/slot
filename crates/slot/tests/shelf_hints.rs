mod common;

use slot::app::{App, ShelfKind};
use slot_gfx::{Draw, TexId, OUT_W};
use slot_input::{Action, Btn};
use slot_ui::shelf_switch_hint_face;

#[test]
fn category_hints_follow_the_screens_where_shoulders_switch_shelves() {
    let root = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::clocked(root.path());
    let mut app = App::boot(root.path());
    let left = TexId::from_raw(9001);
    let right = TexId::from_raw(9002);
    let width = shelf_switch_hint_face(false).w;
    app.set_shelf_switch_hint_faces([(left, width), (right, width)]);
    let hints = |app: &App| {
        let mut out = Vec::new();
        app.draw(&mut out);
        out.into_iter()
            .filter_map(|d| match d {
                Draw::Tex { x, y, tex, .. } if tex == left || tex == right => Some((tex, x, y)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let corners = vec![
        (left, 24.0, 24.0),
        (right, OUT_W as f32 - 24.0 - width as f32, 24.0),
    ];
    assert_eq!(hints(&app), corners);
    app.apply(Action::QuickMenu);
    assert!(hints(&app).is_empty());
    app.apply(Action::QuickMenu);
    assert_eq!(hints(&app), corners);
    app.apply(Action::GbaDown(Btn::Start));
    assert!(app.core_picker().is_some());
    assert!(hints(&app).is_empty());
    app.apply(Action::GbaDown(Btn::B));
    for _ in 0..60 {
        app.update(1.0 / 60.0);
    }
    assert_eq!(hints(&app), corners);
    app.shelf_kind = ShelfKind::Music;
    assert_eq!(hints(&app), corners);
    app.music.flipped = true;
    assert!(hints(&app).is_empty());
    app.music.flipped = false;
    app.music.fullscreen = true;
    assert!(hints(&app).is_empty());
    app.music.fullscreen = false;
    app.shelf_kind = ShelfKind::Console(slot_store::Platform::Gba);
    app.apply(Action::Insert);
    assert!(hints(&app).is_empty());
}
