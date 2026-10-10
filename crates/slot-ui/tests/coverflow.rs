use slot_store::{Cart, Platform};
use slot_ui::{Draw, Shelf, SlotChrome, TexId, OUT_W};

fn shelf(n: usize, platform: Platform) -> Shelf {
    let mut shelf = Shelf::new(
        (0..n)
            .map(|i| Cart {
                platform,
                stem: format!("Game {i}"),
                rom: format!("Game {i}.gba").into(),
                label: None,
                code: String::new(),
                shell: None,
                title: format!("GAME {i}"),
            })
            .collect(),
    );
    shelf.set_faces((0..n).map(TexId::from_raw).collect());
    shelf.set_coverflow(true);
    shelf
}

fn draw(shelf: &Shelf) -> Vec<Draw> {
    let mut out = Vec::new();
    shelf.draw_row(None, 0.0, 0.0, 1.0, &mut out);
    out
}

fn settle(shelf: &mut Shelf) {
    for _ in 0..120 {
        shelf.update(1.0 / 60.0);
    }
}

#[test]
fn coverflow_centres_the_flat_selection_above_inward_facing_neighbours() {
    for platform in Platform::ALL {
        let s = shelf(10, platform);
        let out = draw(&s);
        let Some(Draw::Tex {
            x, w, tex, alpha, ..
        }) = out.last()
        else {
            panic!("the flat selection must be painted last");
        };
        assert_eq!(*tex, TexId::from_raw(0));
        assert_eq!(*alpha, 1.0);
        assert!((x + w / 2.0 - OUT_W as f32 / 2.0).abs() < 0.01);
        for d in &out[..out.len() - 1] {
            let Draw::Yawed {
                x,
                w,
                yaw,
                colour,
                tex,
                ..
            } = d
            else {
                panic!("a neighbouring cart should have perspective");
            };
            let offset = x + w / 2.0 - OUT_W as f32 / 2.0;
            assert!(offset * yaw < 0.0);
            if tex.is_some() {
                assert!(colour[3] < 1.0);
            }
        }
    }
}

#[test]
fn a_press_animates_continuously_through_both_wrap_boundaries() {
    for n in [2, 3, 10] {
        for right in [false, true] {
            let mut s = shelf(n, Platform::Gba);
            s.select(if right { n - 1 } else { 0 });
            let before = draw(&s);
            if right {
                s.right();
            } else {
                s.left();
            }
            assert_eq!(draw(&s), before, "a press replaced the visible carts");
            let initial_yaw = s.selected_yaw();
            s.update(1.0 / 60.0);
            assert!(s.selected_yaw().abs() <= initial_yaw.abs());
            assert_ne!(draw(&s), before);
            settle(&mut s);
            assert!(s.selected_yaw().abs() < 0.001);
            let (x, scale) = s.selected_at();
            assert!((scale - 1.0).abs() < 0.001);
            assert!((x - (OUT_W as f32 - slot_ui::CART_W as f32) / 2.0).abs() < 0.01);
        }
    }
}

#[test]
fn switching_off_restores_the_existing_carousel_without_losing_selection() {
    let mut s = shelf(10, Platform::Gba);
    s.right();
    settle(&mut s);
    s.set_coverflow(false);
    assert_eq!(s.index, 1);
    assert!(draw(&s).iter().all(|d| !matches!(d, Draw::Yawed { .. })));
    s.set_coverflow(true);
    assert_eq!(s.index, 1);
    assert!(draw(&s).iter().any(|d| matches!(d, Draw::Yawed { .. })));
}

#[test]
fn empty_and_single_cart_shelves_stay_flat_and_inert() {
    for n in [0, 1] {
        let mut s = shelf(n, Platform::Gb);
        s.left();
        s.right();
        settle(&mut s);
        assert_eq!(s.index, 0);
        assert_eq!(draw(&s).len(), n);
        assert_eq!(s.selected_yaw(), 0.0);
    }
}

#[test]
fn visible_faces_follow_the_animation_during_a_large_letter_jump() {
    let mut s = shelf(40, Platform::Gba);
    for (i, cart) in s.carts.iter_mut().enumerate() {
        cart.stem = format!("{} Game {i}", if i < 20 { 'A' } else { 'B' });
    }
    let before = s.on_screen();
    s.jump_next_letter();
    assert_eq!(s.index, 20);
    assert_eq!(
        s.on_screen(),
        before,
        "visible face loading jumped ahead of the animation"
    );
    for _ in 0..120 {
        s.update(1.0 / 60.0);
        let visible = s.on_screen();
        for d in draw(&s) {
            let tex = match d {
                Draw::Tex { tex, .. } => tex,
                Draw::Yawed { tex: Some(tex), .. } => tex,
                _ => panic!("a cart with a texture"),
            };
            assert!(visible.iter().any(|i| TexId::from_raw(*i) == tex));
        }
    }
    assert_eq!(s.on_screen()[0], 20);
}

#[test]
fn insertion_hides_every_copy_of_the_selected_cart_and_recedes_the_row() {
    let mut s = shelf(3, Platform::Gba);
    s.right();
    s.update(1.0 / 60.0);
    let mut out = Vec::new();
    s.draw_row(Some("Game 1"), 0.0, 0.5, 1.0, &mut out);
    assert!(!out.is_empty());
    for d in out {
        let Draw::Yawed { tex, colour, .. } = d else {
            panic!("angled neighbour");
        };
        assert_ne!(tex, Some(TexId::from_raw(1)));
        assert!(colour[3] <= 0.5);
    }
    let mut out = Vec::new();
    s.draw_row(Some("Game 1"), 0.0, 1.0, 1.0, &mut out);
    assert!(out.is_empty());
}

#[test]
fn a_moving_selection_keeps_its_pose_as_insertion_begins_then_straightens() {
    let mut s = shelf(10, Platform::Gba);
    s.right();
    s.update(1.0 / 60.0);
    let (rest, scale) = s.selected_at();
    let mut chrome = SlotChrome {
        cart: &s.carts[s.index],
        face: s.face(s.index),
        rest,
        scale,
        seat: 0.0,
        alert: None,
        dim: 0.0,
        screen: 0.0,
        game: false,
    };
    let mut out = Vec::new();
    chrome.draw_with_yaw(s.selected_yaw(), &mut out);
    let Some(Draw::Yawed { x, yaw, .. }) = out
        .iter()
        .find(|d| matches!(d, Draw::Yawed { tex: Some(tex), .. } if *tex == TexId::from_raw(1)))
    else {
        panic!("the insertion lost the cart's angle");
    };
    assert_eq!(*x, rest);
    assert_eq!(*yaw, s.selected_yaw());
    chrome.seat = 1.0;
    out.clear();
    chrome.draw_with_yaw(s.selected_yaw(), &mut out);
    assert!(!out.iter().any(|d| matches!(d, Draw::Yawed { .. })));
}
