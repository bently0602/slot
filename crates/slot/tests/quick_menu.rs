mod common;

use common::{
    app_booting_at, app_booting_with_clock, app_playing_in, tmp_root_with_carts, Clock,
    CLOCK_IS_SET,
};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, Shader, SlotState};
use slot_ui::{
    edge, Draw, Icon, QuickMenuFaces, QuickRow, QuickValue, TexId, MENU_PAD, OUT_W, QUICK_EDGE,
    QUICK_MAIN_PITCH, QUICK_TOP,
};
use tempfile::TempDir;

fn on_carousel_with(state: SlotState) -> (TempDir, App, Clock) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..state
        },
    )
    .expect("write slot.state");
    let (a, clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(
        matches!(a.phase(), Phase::Shelf),
        "not on the carousel: {:?}",
        a.phase()
    );
    (d, a, clock)
}

fn on_carousel() -> (TempDir, App, Clock) {
    on_carousel_with(SlotState::default())
}

#[test]
fn screen_off_blanks_the_shelf_and_wakes_without_activating_a_cart() {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    let (d, mut a, _) = on_carousel();
    let (power, backlight) = common::panel(d.path(), Duration::from_secs(180));
    a.set_power(power);
    let brightness = a.brightness();
    let selected = a.selected_stem().unwrap().to_string();
    open_at(&mut a, QuickRow::ScreenOff);
    assert!(a.quick_value(QuickRow::ScreenOff).is_none());
    press(&mut a, Btn::A);
    assert!(a.screen_off());
    assert!(matches!(a.phase(), Phase::Shelf));
    assert_eq!(backlight.load(Ordering::Relaxed), 0);
    assert_eq!(a.brightness(), brightness);
    let mut draws = Vec::new();
    a.draw(&mut draws);
    assert_eq!(draws.len(), 1);
    assert!(matches!(
        draws[0],
        Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: 720.0,
            h: 480.0,
            colour: [0.0, 0.0, 0.0, 1.0]
        }
    ));
    press(&mut a, Btn::A);
    assert!(!a.screen_off());
    assert!(matches!(a.phase(), Phase::Shelf));
    assert_eq!(a.selected_stem(), Some(selected.as_str()));
    assert_eq!(backlight.load(Ordering::Relaxed), brightness);
    open_at(&mut a, QuickRow::ScreenOff);
    press(&mut a, Btn::A);
    a.apply(Action::PowerPress);
    a.apply(Action::PowerTap);
    assert!(!a.screen_off());
    assert!(matches!(a.phase(), Phase::Shelf));
    open_at(&mut a, QuickRow::ScreenOff);
    press(&mut a, Btn::A);
    a.apply(Action::LidClose);
    assert!(matches!(a.phase(), Phase::Doze { .. }));
    a.apply(Action::LidOpen);
    assert!(!a.screen_off());
    assert_eq!(backlight.load(Ordering::Relaxed), brightness);
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn open_at(a: &mut App, row: QuickRow) {
    a.apply(Action::QuickMenu);
    let parent = row.parent();
    for _ in 0..parent.unwrap_or(row).position() {
        press(a, Btn::Down);
    }
    if parent.is_some() {
        press(a, Btn::A);
        for _ in 0..row.position() {
            press(a, Btn::Down);
        }
    }
    assert_eq!(a.quick_menu(), Some(row), "the bar never reached {row:?}");
}

#[test]
fn menu_opens_the_quick_menu_with_its_top_row_selected_every_time() {
    let (_d, mut a, _) = on_carousel();
    a.apply(Action::QuickMenu);
    assert_eq!(a.quick_menu(), Some(QuickRow::MAIN[0]));
    press(&mut a, Btn::Down);
    a.apply(Action::QuickMenu);
    a.apply(Action::QuickMenu);
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::MAIN[0]),
        "the menu opened where it was last left"
    );
}

#[test]
fn menu_or_b_closes_it_back_onto_the_carousel_where_you_were() {
    for close in [Action::QuickMenu, Action::GbaDown(Btn::B)] {
        let (_d, mut a, _) = on_carousel();
        press(&mut a, Btn::Right);
        assert_eq!(
            a.selected_stem(),
            Some("Fusion"),
            "the carousel never moved"
        );
        a.apply(Action::QuickMenu);
        a.apply(close);
        assert!(
            matches!(a.phase(), Phase::Shelf),
            "{close:?} left {:?}",
            a.phase()
        );
        assert_eq!(a.selected_stem(), Some("Fusion"), "{close:?} lost the cart");
    }
}

#[test]
fn device_info_requests_a_system_app_without_changing_the_cartridges() {
    let (d, mut a, _) = on_carousel();
    let app_dir = d.path().join("System/device-info");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("device-info"), "test app").unwrap();
    std::fs::write(d.path().join("System/device-info.sh"), "test launcher").unwrap();
    let selected = a.selected_stem().unwrap().to_owned();
    let carts = a.carts().count();
    open_at(&mut a, QuickRow::DeviceInfo);
    press(&mut a, Btn::Left);
    assert_eq!(a.take_device_info_request(), None);
    press(&mut a, Btn::A);
    assert_eq!(a.take_device_info_request(), Some(d.path().to_path_buf()));
    assert_eq!(a.take_device_info_request(), None);
    a.return_to_device_info_menu();
    assert_eq!(a.quick_menu(), Some(QuickRow::DeviceInfo));
    assert_eq!(a.selected_stem(), Some(selected.as_str()));
    assert_eq!(a.carts().count(), carts);
    assert_eq!(a.quick_value(QuickRow::DeviceInfo), None);
}

#[test]
fn missing_device_info_keeps_the_system_menu_open() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::DeviceInfo);
    press(&mut a, Btn::A);
    assert_eq!(a.take_device_info_request(), None);
    assert_eq!(a.quick_menu(), Some(QuickRow::DeviceInfo));
}

#[test]
fn the_quick_menu_is_only_on_the_carousel() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "MENU raised the quick menu over a game: {:?}",
        a.phase()
    );
    assert_eq!(a.quick_menu(), None);
}

#[test]
fn up_and_down_move_the_bar_and_wrap_at_the_ends() {
    let (_d, mut a, _) = on_carousel();
    a.apply(Action::QuickMenu);
    press(&mut a, Btn::Up);
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::About),
        "up from the top did not wrap to the bottom"
    );
    for want in [
        QuickRow::Screen,
        QuickRow::ScreenOff,
        QuickRow::VideoOut,
        QuickRow::Controllers,
        QuickRow::Bluetooth,
        QuickRow::Game,
        QuickRow::Music,
        QuickRow::Battery,
        QuickRow::DateTime,
        QuickRow::DeviceInfo,
        QuickRow::About,
        QuickRow::Screen,
    ] {
        press(&mut a, Btn::Down);
        assert_eq!(a.quick_menu(), Some(want));
    }
}

#[test]
fn fast_forward_steps_through_its_speeds_wraps_at_both_ends_and_saves_each_one() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::FastForward);
    for (btn, want) in [
        (Btn::Left, 4),
        (Btn::Left, 3),
        (Btn::Left, 2),
        (Btn::Left, 6),
        (Btn::Right, 2),
        (Btn::Right, 3),
        (Btn::Right, 4),
        (Btn::Right, 6),
        (Btn::Right, 2),
    ] {
        press(&mut a, btn);
        assert_eq!(a.ff_speed(), want, "{btn:?}");
        assert_eq!(read_slot_state(d.path()).ff_speed, want, "not on the card");
        assert_eq!(
            a.quick_value(QuickRow::FastForward),
            QuickValue::speed(want)
        );
    }
}

#[test]
fn rumble_and_fast_forward_sound_flip_on_either_arrow_and_save() {
    let (d, mut a, _) = on_carousel();
    let card = |d: &TempDir| {
        let s = read_slot_state(d.path());
        (s.ff_sound, s.rumble)
    };
    open_at(&mut a, QuickRow::FastForwardSound);
    press(&mut a, Btn::Left);
    assert_eq!(card(&d), (true, true));
    assert_eq!(
        a.quick_value(QuickRow::FastForwardSound),
        Some(QuickValue::On)
    );
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, true));
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, false));
    assert!(!a.rumble_enabled());
    assert_eq!(a.quick_value(QuickRow::Rumble), Some(QuickValue::Off));
    press(&mut a, Btn::Left);
    assert_eq!(card(&d), (false, true));
}

#[test]
fn colour_correction_flips_on_either_arrow_and_saves() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::ColourCorrection);
    assert!(
        !a.colour_correction(),
        "the row did not open on the default, which is off"
    );
    assert_eq!(
        a.quick_value(QuickRow::ColourCorrection),
        Some(QuickValue::Off)
    );
    for (btn, want) in [
        (Btn::Right, true),
        (Btn::Left, false),
        (Btn::Left, true),
        (Btn::Right, false),
    ] {
        press(&mut a, btn);
        assert_eq!(a.colour_correction(), want, "{btn:?}");
        assert_eq!(
            read_slot_state(d.path()).colour_correction,
            want,
            "{btn:?} never reached the card"
        );
        assert_eq!(
            a.quick_value(QuickRow::ColourCorrection),
            Some(QuickValue::flag(want))
        );
    }
}

#[test]
fn the_game_page_flips_eject_save_turbo_and_rewind_and_saves_each() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::Rewind);
    let card = |d: &TempDir| {
        let s = read_slot_state(d.path());
        (s.eject_save, s.turbo, s.rewind)
    };
    assert_eq!(card(&d), (true, true, true));
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (true, true, false));
    assert!(!a.may_rewind(), "rewind still offered with it off");
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Left);
    assert_eq!(card(&d), (true, false, false));
    assert!(!a.turbo());
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, false, false));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Game));
}

#[test]
fn screen_opens_a_page_of_its_own_and_b_comes_back_to_it() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::Screen);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::GbaShader));
    for want in [
        QuickRow::GbShader,
        QuickRow::SnesShader,
        QuickRow::PsxShader,
        QuickRow::ColourCorrection,
        QuickRow::GbPalettes,
        QuickRow::CoverFlow,
        QuickRow::GbaShader,
    ] {
        press(&mut a, Btn::Down);
        assert_eq!(
            a.quick_menu(),
            Some(want),
            "the page did not wrap on itself"
        );
    }
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Screen));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), None);
}

#[test]
fn each_platform_keeps_its_own_shader_wraps_and_starts_on_grid_and_simpletex() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::GbaShader);
    assert_eq!(a.quick_value(QuickRow::GbaShader), Some(QuickValue::Grid));
    assert_eq!(
        a.quick_value(QuickRow::GbShader),
        Some(QuickValue::Simpletex)
    );
    for (btn, want, shown) in [
        (Btn::Right, Shader::Dot, QuickValue::Dot),
        (Btn::Right, Shader::Simpletex, QuickValue::Simpletex),
        (Btn::Right, Shader::Off, QuickValue::Off),
        (Btn::Left, Shader::Simpletex, QuickValue::Simpletex),
        (Btn::Left, Shader::Dot, QuickValue::Dot),
        (Btn::Left, Shader::Grid, QuickValue::Grid),
        (Btn::Left, Shader::Lcd3x, QuickValue::Lcd3x),
        (Btn::Left, Shader::Off, QuickValue::Off),
    ] {
        press(&mut a, btn);
        assert_eq!(a.quick_value(QuickRow::GbaShader), Some(shown), "{btn:?}");
        assert_eq!(
            read_slot_state(d.path()).shader_gba,
            want,
            "{btn:?} never reached the card"
        );
    }
    assert_eq!(read_slot_state(d.path()).shader_gb, Shader::Simpletex);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Left);
    assert_eq!(read_slot_state(d.path()).shader_gb, Shader::Dot);
    assert_eq!(read_slot_state(d.path()).shader_gba, Shader::Off);
}

#[test]
fn colour_correction_leaves_the_settings_around_it_alone() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::ColourCorrection);
    press(&mut a, Btn::Right);
    let s = read_slot_state(d.path());
    assert!(s.colour_correction, "the row never took");
    assert_eq!(
        (s.ff_speed, s.ff_sound, s.rumble),
        (
            SlotState::default().ff_speed,
            SlotState::default().ff_sound,
            SlotState::default().rumble
        ),
        "the row reached a setting that is not its own"
    );
}

#[test]
fn all_system_shader_rows_offer_every_effect_and_persist_independently() {
    let (d, mut app, _) = on_carousel_with(SlotState {
        shader_gba: Shader::Off,
        shader_gb: Shader::Off,
        ..SlotState::default()
    });
    for row in [
        QuickRow::GbaShader,
        QuickRow::GbShader,
        QuickRow::SnesShader,
        QuickRow::PsxShader,
    ] {
        let before = read_slot_state(d.path());
        open_at(&mut app, row);
        for (button, expected) in [
            (Btn::Right, QuickValue::Lcd3x),
            (Btn::Right, QuickValue::Grid),
            (Btn::Right, QuickValue::Dot),
            (Btn::Right, QuickValue::Simpletex),
            (Btn::Right, QuickValue::Off),
            (Btn::Left, QuickValue::Simpletex),
            (Btn::Left, QuickValue::Dot),
            (Btn::Left, QuickValue::Grid),
            (Btn::Left, QuickValue::Lcd3x),
            (Btn::Left, QuickValue::Off),
        ] {
            press(&mut app, button);
            assert_eq!(app.quick_value(row), Some(expected), "{row:?}");
        }
        assert_eq!(
            read_slot_state(d.path()),
            before,
            "cycling {row:?} changed other preferences"
        );
        press(&mut app, Btn::Right);
        let after = read_slot_state(d.path());
        let mut want = before;
        match row {
            QuickRow::GbaShader => want.shader_gba = Shader::Lcd3x,
            QuickRow::GbShader => want.shader_gb = Shader::Lcd3x,
            QuickRow::SnesShader => want.shader_snes = Shader::Lcd3x,
            QuickRow::PsxShader => want.shader_psx = Shader::Lcd3x,
            _ => unreachable!(),
        }
        assert_eq!(after, want);
        app.apply(Action::QuickMenu);
        app.apply(Action::QuickMenu);
    }
    let mut rebooted = App::boot(d.path());
    for platform in slot_store::Platform::ALL {
        rebooted.set_platform(platform);
        assert_eq!(rebooted.screen_shader(), Shader::Lcd3x, "{platform:?}");
    }
}

#[test]
fn shader_shortcuts_apply_and_save_on_every_platform() {
    let (d, mut app, _) = on_carousel();
    app.apply(Action::Insert);
    for platform in slot_store::Platform::ALL {
        app.set_platform(platform);
        let original = app.screen_shader();
        let mut expected = original;
        for action in [Action::ShaderNext, Action::ShaderPrev] {
            for _ in Shader::ALL {
                expected = expected.step(&Shader::ALL, action == Action::ShaderNext);
                app.apply(action);
                assert_eq!(app.screen_shader(), expected, "{platform:?}: {action:?}");
                let saved = read_slot_state(d.path());
                let stored = match platform {
                    slot_store::Platform::Gba => saved.shader_gba,
                    slot_store::Platform::Gb | slot_store::Platform::Gbc => saved.shader_gb,
                    slot_store::Platform::Snes => saved.shader_snes,
                    slot_store::Platform::Psx => saved.shader_psx,
                };
                assert_eq!(stored, expected, "{platform:?} preference did not persist");
            }
            assert_eq!(app.screen_shader(), original);
        }
    }
}

#[test]
fn the_arrows_change_nothing_on_a_row_that_opens() {
    let (d, mut a, _) = on_carousel();
    let before = std::fs::read(d.path().join("Config/slot.state")).expect("read slot.state");
    for row in [QuickRow::DateTime, QuickRow::About] {
        open_at(&mut a, row);
        press(&mut a, Btn::Left);
        press(&mut a, Btn::Right);
        assert_eq!(a.quick_menu(), Some(row), "an arrow left {row:?}");
        a.apply(Action::QuickMenu);
    }
    assert_eq!(
        std::fs::read(d.path().join("Config/slot.state")).expect("read slot.state"),
        before,
        "an arrow on a row that opens wrote the card"
    );
}

#[test]
fn a_on_about_opens_the_label_and_b_comes_back_to_the_menu() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::About);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::About), "{:?}", a.phase());
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::About));
}

#[test]
fn a_on_date_and_time_opens_the_clock_at_the_time_it_already_has() {
    let (_d, mut a, _) = on_carousel_with(SlotState {
        utc_offset_min: -300,
        ..SlotState::default()
    });
    open_at(&mut a, QuickRow::DateTime);
    press(&mut a, Btn::A);
    let picker = a.picker().expect("A on Date & Time did not open the clock");
    assert_eq!(picker.offset_min(), -300, "the offset started over");
    assert_eq!(
        picker.secs(),
        CLOCK_IS_SET - CLOCK_IS_SET % 60,
        "the picker does not show the minute the clock is in"
    );
}

#[test]
fn confirming_the_clock_from_the_menu_sets_it_and_comes_back_to_the_menu() {
    let (d, mut a, clock) = on_carousel_with(SlotState {
        utc_offset_min: -300,
        ..SlotState::default()
    });
    open_at(&mut a, QuickRow::DateTime);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Up);
    for _ in 0..5 {
        press(&mut a, Btn::Right);
    }
    press(&mut a, Btn::Down);
    let changed = a.picker().expect("not on the clock").secs() - (CLOCK_IS_SET - CLOCK_IS_SET % 60);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::DateTime), "{:?}", a.phase());
    assert_eq!(
        clock.get(),
        CLOCK_IS_SET + changed,
        "the platform clock was not set"
    );
    let s = read_slot_state(d.path());
    assert_eq!(s.utc_offset_min, -330, "the offset was not saved");
    assert!(s.clock_set);
}

#[test]
fn b_on_the_clock_from_the_menu_comes_back_without_changing_anything() {
    let (d, mut a, clock) = on_carousel_with(SlotState {
        utc_offset_min: -300,
        ..SlotState::default()
    });
    open_at(&mut a, QuickRow::DateTime);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Up);
    for _ in 0..5 {
        press(&mut a, Btn::Right);
    }
    press(&mut a, Btn::Down);
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::DateTime), "{:?}", a.phase());
    assert_eq!(clock.get(), CLOCK_IS_SET, "B set the clock");
    assert_eq!(
        read_slot_state(d.path()).utc_offset_min,
        -300,
        "B saved the offset"
    );
}

#[test]
fn the_first_boot_clock_still_has_no_way_back() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut a, _) = app_booting_with_clock(d.path());
    press(&mut a, Btn::B);
    a.apply(Action::QuickMenu);
    assert!(
        matches!(a.phase(), Phase::SetClock { .. }),
        "left the first boot clock: {:?}",
        a.phase()
    );
}

#[test]
fn brightness_and_volume_still_answer_over_the_quick_menu() {
    let (d, mut a, _) = on_carousel();
    let icons: Vec<TexId> = (0..Icon::ALL.len())
        .map(|i| TexId::from_raw(700 + i))
        .collect();
    a.set_icon_faces(icons.clone());
    open_at(&mut a, QuickRow::FastForward);
    let before = read_slot_state(d.path());
    a.apply(Action::BrightnessUp);
    a.apply(Action::VolumeDown);
    a.flush_state();
    let after = read_slot_state(d.path());
    assert_eq!(
        (after.brightness, after.volume),
        (before.brightness + 1, before.volume - 5)
    );
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::FastForward),
        "a level moved the menu"
    );
    let out = frame(&a);
    assert!(
        out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if icons.contains(&tex))),
        "the level's bar is not drawn over the menu"
    );
}

fn fake_faces(a: &mut App) {
    let id = TexId::from_raw;
    a.set_quick_menu_faces(QuickMenuFaces {
        labels: (0..QuickRow::ALL.len())
            .map(|i| (id(100 + i), 200, 40))
            .collect(),
        values: (0..QuickValue::ALL.len())
            .map(|i| [(id(200 + i), 60, 40), (id(210 + i), 60, 40)])
            .collect(),
        carets: [(id(300), 10, 40), (id(301), 10, 40)],
        legend: [(id(400), 70), (id(401), 110), (id(402), 80)],
    });
    a.set_quick_clock_faces((id(500), 150, 40), (id(501), 150, 40));
}

fn value(v: QuickValue, lit: bool) -> usize {
    if lit {
        210 + v.index()
    } else {
        200 + v.index()
    }
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

fn placed(out: &[Draw], id: usize) -> Option<[f32; 4]> {
    out.iter().find_map(|d| match *d {
        Draw::Tex {
            x, y, w, h, tex, ..
        } if tex == TexId::from_raw(id) => Some([x, y, w, h]),
        _ => None,
    })
}

fn drawn(out: &[Draw], id: usize) -> bool {
    placed(out, id).is_some()
}

#[test]
fn the_legend_says_change_on_a_value_row_and_open_on_a_row_that_opens() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    a.apply(Action::QuickMenu);
    for row in QuickRow::MAIN {
        assert_eq!(a.quick_menu(), Some(row));
        let out = frame(&a);
        assert!(drawn(&out, 400), "no B BACK on {row:?}");
        assert_eq!(drawn(&out, 401), !row.opens(), "CHANGE on {row:?}");
        assert_eq!(drawn(&out, 402), row.opens(), "OPEN on {row:?}");
        press(&mut a, Btn::Down);
    }
}

#[test]
fn the_arrows_stand_only_around_the_selected_rows_value() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, QuickRow::FastForward);
    let out = frame(&a);
    assert!(
        drawn(&out, 300) && drawn(&out, 301),
        "no arrows on Fast Forward"
    );
    assert!(
        drawn(&out, value(QuickValue::Speed6, true)),
        "6× is not lit"
    );
    assert!(
        drawn(&out, value(QuickValue::Off, false)),
        "sound's OFF is not grey"
    );
    assert!(
        drawn(&out, value(QuickValue::On, false)),
        "rumble's ON is not grey"
    );
    press(&mut a, Btn::B);
    let out = frame(&a);
    assert!(drawn(&out, 500), "the date and time is not grey");

    for _ in QuickRow::Game.position()..QuickRow::DateTime.position() {
        press(&mut a, Btn::Down);
    }
    let out = frame(&a);
    assert!(
        !drawn(&out, 300) && !drawn(&out, 301),
        "arrows on a row that opens"
    );
    assert!(drawn(&out, 501), "the date and time in hand is not lit");
}

#[test]
fn the_bar_runs_edge_to_edge_behind_the_selected_row() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    a.apply(Action::QuickMenu);
    for row in QuickRow::MAIN {
        let bars: Vec<_> = frame(&a)
            .into_iter()
            .filter_map(|d| match d {
                Draw::Rect { x, y, w, h, colour } if colour == edge() => Some([x, y, w, h]),
                _ => None,
            })
            .collect();
        let top = QUICK_TOP + QUICK_MAIN_PITCH * row.position() as f32;
        assert_eq!(
            bars,
            vec![[0.0, top + 4.0, OUT_W as f32, QUICK_MAIN_PITCH - 8.0]],
            "{row:?}"
        );
        press(&mut a, Btn::Down);
    }
}

#[test]
fn labels_start_and_values_end_thirty_two_pixels_in() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, QuickRow::FastForward);
    let out = frame(&a);
    let right = OUT_W as f32 - QUICK_EDGE;
    for row in QuickRow::GAME {
        let [x, ..] = placed(&out, 100 + row.index()).expect("a label was not drawn");
        assert_eq!(x + MENU_PAD as f32, QUICK_EDGE, "{row:?}'s label");
    }
    let [x, _, w, _] = placed(&out, value(QuickValue::On, false)).expect("rumble's value");
    assert_eq!(x + w - MENU_PAD as f32, right, "an unselected value");
    let [x, _, w, _] = placed(&out, 301).expect("the right arrow");
    assert_eq!(x + w, right, "the arrow around the selected value");
}

#[test]
fn only_the_clock_from_the_menu_offers_b_back() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    a.set_clock_faces(TexId::from_raw(600), TexId::from_raw(601));
    open_at(&mut a, QuickRow::DateTime);
    press(&mut a, Btn::A);
    let out = frame(&a);
    assert!(drawn(&out, 601), "the clock lost its own key");
    assert!(
        drawn(&out, 400),
        "the clock from the menu does not offer B BACK"
    );

    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut first, _) = app_booting_with_clock(d.path());
    fake_faces(&mut first);
    first.set_clock_faces(TexId::from_raw(600), TexId::from_raw(601));
    let out = frame(&first);
    assert!(drawn(&out, 601), "the first boot clock lost its key");
    assert!(
        !drawn(&out, 400),
        "the first boot clock offers a way back it does not have"
    );
}

#[test]
fn gb_palettes_flips_on_either_arrow_and_saves() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::GbPalettes);
    assert_eq!(a.quick_value(QuickRow::GbPalettes), Some(QuickValue::Off));
    for (btn, want) in [(Btn::Right, true), (Btn::Left, false), (Btn::Left, true)] {
        press(&mut a, btn);
        assert_eq!(
            read_slot_state(d.path()).gb_palettes,
            want,
            "{btn:?} never reached the card"
        );
        assert_eq!(
            a.quick_value(QuickRow::GbPalettes),
            Some(QuickValue::flag(want))
        );
    }
}

#[test]
fn coverflow_toggle_changes_the_shelf_and_survives_a_restart() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::CoverFlow);
    assert_eq!(a.quick_value(QuickRow::CoverFlow), Some(QuickValue::Off));
    for (btn, want) in [(Btn::Right, true), (Btn::Left, false), (Btn::Left, true)] {
        press(&mut a, btn);
        assert_eq!(read_slot_state(d.path()).coverflow, want);
        assert_eq!(
            a.quick_value(QuickRow::CoverFlow),
            Some(QuickValue::flag(want))
        );
        a.apply(Action::QuickMenu);
        a.apply(Action::QuickMenu);
        let mut out = Vec::new();
        a.draw(&mut out);
        assert_eq!(out.iter().any(|d| matches!(d, Draw::Yawed { .. })), want);
        assert_eq!(a.selected_stem(), Some("Emerald"));
        open_at(&mut a, QuickRow::CoverFlow);
    }
    let (rebooted, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert_eq!(
        rebooted.quick_value(QuickRow::CoverFlow),
        Some(QuickValue::On)
    );
    let mut out = Vec::new();
    rebooted.draw(&mut out);
    assert!(out.iter().any(|d| matches!(d, Draw::Yawed { .. })));
}

#[test]
fn coverflow_toggle_applies_to_every_platform_shelf() {
    let (d, _, _) = on_carousel();
    for stem in ["Tetris", "Zelda"] {
        common::write_gb_cart(&d, stem, &stem.to_uppercase());
        let mut rom = vec![0u8; 0x150];
        rom[0x134..0x134 + stem.len()].copy_from_slice(stem.as_bytes());
        rom[0x143] = 0x80;
        std::fs::write(d.path().join(format!("Games/GBC/{stem}.gbc")), rom).unwrap();
    }
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    open_at(&mut a, QuickRow::CoverFlow);
    press(&mut a, Btn::Right);
    a.apply(Action::QuickMenu);
    a.apply(Action::QuickMenu);
    for _ in 0..3 {
        let mut out = Vec::new();
        a.draw(&mut out);
        assert!(out.iter().any(|d| matches!(d, Draw::Yawed { .. })));
        press(&mut a, Btn::R1);
    }
    open_at(&mut a, QuickRow::CoverFlow);
    press(&mut a, Btn::Left);
    a.apply(Action::QuickMenu);
    a.apply(Action::QuickMenu);
    for _ in 0..3 {
        let mut out = Vec::new();
        a.draw(&mut out);
        assert!(!out.iter().any(|d| matches!(d, Draw::Yawed { .. })));
        press(&mut a, Btn::R1);
    }
}

#[test]
fn video_out_changes_persist_and_b_returns_to_its_parent() {
    let (d, mut a, clock) = on_carousel();
    open_at(&mut a, QuickRow::VideoOut);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::HdmiOutput));
    assert_eq!(a.quick_value(QuickRow::HdmiOutput), Some(QuickValue::Auto));
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Left);
    let saved = read_slot_state(d.path());
    assert!(!saved.hdmi_auto);
    assert!(saved.hdmi_1080p);
    assert_eq!(saved.hdmi_scale, slot_store::HdmiScale::Integer);
    assert_eq!(saved.hdmi_margin, 10);
    press(&mut a, Btn::Down);
    assert_eq!(a.quick_menu(), Some(QuickRow::DockedLid));
    assert_eq!(a.quick_value(QuickRow::DockedLid), Some(QuickValue::On));
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Down);
    assert_eq!(a.quick_menu(), Some(QuickRow::AudioOutput));
    assert_eq!(a.quick_value(QuickRow::AudioOutput), Some(QuickValue::Auto));
    press(&mut a, Btn::Left);
    assert_eq!(a.quick_value(QuickRow::AudioOutput), Some(QuickValue::Hdmi));
    let saved = read_slot_state(d.path());
    assert!(!saved.docked_lid);
    assert_eq!(saved.audio_output, slot_store::AudioOutput::Hdmi);
    press(&mut a, Btn::Down);
    assert_eq!(a.quick_menu(), Some(QuickRow::HdmiOutput));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::VideoOut));
    let (rebooted, _) = app_booting_at(d.path(), clock.get());
    assert_eq!(
        rebooted.quick_value(QuickRow::HdmiOutput),
        Some(QuickValue::Internal)
    );
    assert_eq!(
        rebooted.quick_value(QuickRow::HdmiResolution),
        Some(QuickValue::P1080)
    );
    assert_eq!(
        rebooted.hdmi_scaling(),
        (slot_store::HdmiScale::Integer, 10)
    );
}
