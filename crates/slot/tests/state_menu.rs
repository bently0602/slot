mod common;

use common::{app_playing_in, app_playing_with, tmp_root_with_carts, CoreSnapshot};
use slot::app::{Phase, StateSlotMode};
use slot::run_ahead::{run_ahead_for, write_run_ahead, RUN_AHEAD_FILE};
use slot_input::{Action, Btn};
use slot_store::{Core, Platform, StateRing};

fn states(root: &std::path::Path) -> usize {
    StateRing::new(root, Platform::Gba, Core::Mgba, "Emerald")
        .list()
        .expect("list")
        .len()
}

#[test]
fn a_menu_tap_while_playing_opens_the_state_menu_and_holds_the_game() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert_eq!(a.state_menu_row(), None);
    a.apply(Action::QuickMenu);
    assert_eq!(a.state_menu_row(), Some(0));
    assert!(a.game_menu_open(), "the game is held behind the menu");
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    a.apply(Action::GbaDown(Btn::Up));
    assert_eq!(a.state_menu_row(), Some(6));
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::Down));
    assert_eq!(a.state_menu_row(), Some(1));
}

#[test]
fn menu_playback_is_held_stays_on_its_row_and_ignores_navigation_until_release() {
    use slot::app::Playback;
    for platform in Platform::ALL {
        for (row, playback) in [(4, Playback::Rewind), (5, Playback::FastForward)] {
            let d = tmp_root_with_carts(&["Emerald"]);
            let mut a = app_playing_in(d.path(), "Emerald");
            a.set_platform(platform);
            a.apply(Action::QuickMenu);
            for _ in 0..row {
                a.apply(Action::GbaDown(Btn::Down));
            }
            a.apply(Action::GbaDown(Btn::A));
            assert_eq!(a.playback(), Some(playback));
            for button in [Btn::Up, Btn::Down, Btn::Left, Btn::Right] {
                a.apply(Action::GbaDown(button));
                assert_eq!(a.state_menu_row(), Some(row));
                assert_eq!(a.playback(), Some(playback));
            }
            a.apply(Action::GbaUp(Btn::A));
            assert_eq!(a.playback(), None);
            assert_eq!(a.state_menu_row(), Some(row));
            assert!(a.game_menu_open());
            a.apply(Action::GbaDown(Btn::B));
            assert!(!a.game_menu_open());
        }
    }
}

#[test]
fn playback_can_be_cancelled_without_an_a_release_and_never_resumes_after_sleep() {
    use slot::app::Playback;
    for cancel in [
        Action::QuickMenu,
        Action::GbaDown(Btn::B),
        Action::PowerTap,
        Action::PowerHold,
        Action::LidClose,
    ] {
        let d = tmp_root_with_carts(&["Emerald"]);
        let mut a = app_playing_in(d.path(), "Emerald");
        a.apply(Action::QuickMenu);
        for _ in 0..5 {
            a.apply(Action::GbaDown(Btn::Down));
        }
        a.apply(Action::GbaDown(Btn::A));
        assert_eq!(a.playback(), Some(Playback::FastForward));
        a.apply(cancel);
        assert_eq!(a.playback(), None);
        if matches!(cancel, Action::PowerTap | Action::LidClose) {
            assert!(matches!(a.phase(), Phase::Doze { .. }));
            a.apply(Action::PowerTap);
        }
        assert_eq!(a.playback(), None);
    }
}

#[test]
fn seven_menu_rows_fit_on_the_screen_and_playback_shows_the_game() {
    use slot_ui::{Draw, TexId, OUT_H};
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_game_ready(true);
    a.set_state_menu_faces(
        (10..16).map(|id| (TexId::from_raw(id), 400, 36)).collect(),
        (20..23).map(|id| (TexId::from_raw(id), 400, 36)).collect(),
    );
    a.apply(Action::QuickMenu);
    let mut draws = Vec::new();
    a.draw(&mut draws);
    let rows: Vec<_> = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Tex { tex, y, h, .. } => {
                assert!(*y >= 0.0 && *y + *h <= OUT_H as f32);
                Some(*tex)
            }
            _ => None,
        })
        .collect();
    assert_eq!(rows, [10, 11, 20, 13, 14, 15, 12].map(TexId::from_raw));
    for _ in 0..4 {
        a.apply(Action::GbaDown(Btn::Down));
    }
    a.apply(Action::GbaDown(Btn::A));
    draws.clear();
    a.draw(&mut draws);
    assert!(draws.iter().any(|d| matches!(d, Draw::Game)));
}

#[test]
fn save_state_opens_four_slots_and_replaces_only_the_chosen_slot() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    for slot in 0..4 {
        a.apply(Action::QuickMenu);
        a.apply(Action::GbaDown(Btn::A));
        assert_eq!(a.state_slot_menu(), Some((StateSlotMode::Save, 0)));
        assert!(a.game_menu_open());
        for _ in 0..slot {
            a.apply(Action::GbaDown(Btn::Down));
        }
        a.apply(Action::GbaDown(Btn::A));
        assert_eq!(a.state_slot_menu(), None);
        assert_eq!(a.state_menu_row(), None);
        assert!(!a.game_menu_open());
        assert_eq!(ring.read_slot(slot).unwrap().unwrap(), vec![9; 1024]);
    }
    assert_eq!(states(d.path()), 0, "manual slots leaked into the history");
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        a.state_slot_labels(),
        [
            "Slot 1: Saved",
            "Slot 2: Saved",
            "Slot 3: Saved",
            "Slot 4: Saved",
            "Back"
        ]
    );
    a.apply(Action::GbaDown(Btn::A));
    for slot in 0..4 {
        assert!(ring.read_slot(slot).unwrap().is_some());
    }
}

#[test]
fn load_state_offers_four_slots_and_empty_slots_keep_the_game_paused() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.state_slot_menu(), Some((StateSlotMode::Load, 0)));
    assert_eq!(
        a.state_slot_labels(),
        [
            "Slot 1: Empty",
            "Slot 2: Empty",
            "Slot 3: Empty",
            "Slot 4: Empty",
            "Saved History",
            "Back"
        ]
    );
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        a.state_slot_menu(),
        Some((StateSlotMode::Load, 0)),
        "nothing to load yet"
    );
    assert!(a.game_menu_open());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn old_timestamped_states_remain_accessible_from_load_state() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    open_slots(&mut a, StateSlotMode::Load, 4);
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.state_menu_row(), None);
    assert_eq!(a.state_slot_menu(), None);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    assert_eq!(a.polaroid_entries().len(), 1);
}

fn open_slots(a: &mut slot::app::App, mode: StateSlotMode, slot: usize) {
    a.apply(Action::QuickMenu);
    if mode == StateSlotMode::Load {
        a.apply(Action::GbaDown(Btn::Down));
    }
    a.apply(Action::GbaDown(Btn::A));
    for _ in 0..slot {
        a.apply(Action::GbaDown(Btn::Down));
    }
}

#[test]
fn loading_each_slot_restores_its_own_state_after_reboot() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let core = CoreSnapshot::new();
    let mut a = app_playing_with(d.path(), "Emerald", core.boxed());
    let mut saved = Vec::new();
    for slot in 0..4 {
        core.run_frame();
        saved.push(core.bytes());
        open_slots(&mut a, StateSlotMode::Save, slot);
        a.apply(Action::GbaDown(Btn::A));
    }
    drop(a);
    let mut rebooted = app_playing_with(d.path(), "Emerald", core.boxed());
    for (slot, state) in saved.iter().enumerate() {
        core.run_frame();
        open_slots(&mut rebooted, StateSlotMode::Load, slot);
        rebooted.apply(Action::GbaDown(Btn::A));
        assert_eq!(&core.bytes(), state);
        assert!(!rebooted.game_menu_open());
        assert!(matches!(rebooted.phase(), Phase::Playing { .. }));
    }
}

#[test]
fn overwriting_a_slot_and_undoing_it_preserves_the_other_three() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let core = CoreSnapshot::new();
    let mut a = app_playing_with(d.path(), "Emerald", core.boxed());
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    for slot in 0..4 {
        core.run_frame();
        open_slots(&mut a, StateSlotMode::Save, slot);
        a.apply(Action::GbaDown(Btn::A));
    }
    let saved: Vec<_> = (0..4)
        .map(|slot| ring.read_slot(slot).unwrap().unwrap())
        .collect();
    core.run_frame();
    open_slots(&mut a, StateSlotMode::Save, 2);
    a.apply(Action::GbaDown(Btn::A));
    assert_ne!(ring.read_slot(2).unwrap().unwrap(), saved[2]);
    for slot in [0, 1, 3] {
        assert_eq!(ring.read_slot(slot).unwrap().unwrap(), saved[slot]);
    }
    open_slots(&mut a, StateSlotMode::Load, 2);
    a.apply(Action::GbaDown(Btn::X));
    assert_eq!(ring.read_slot(2).unwrap().unwrap(), saved[2]);
    assert_eq!(a.state_slot_menu(), Some((StateSlotMode::Load, 2)));
    assert!(!a.undo_available(0));
}

#[test]
fn undoing_a_first_slot_save_makes_it_empty_again() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    open_slots(&mut a, StateSlotMode::Save, 3);
    a.apply(Action::GbaDown(Btn::A));
    open_slots(&mut a, StateSlotMode::Load, 3);
    a.apply(Action::GbaDown(Btn::X));
    assert_eq!(a.state_slot_labels()[3], "Slot 4: Empty");
    assert_eq!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_slot(3)
            .unwrap(),
        None
    );
}

#[test]
fn undoing_a_manual_load_restores_the_game_before_that_load() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let core = CoreSnapshot::new();
    let mut a = app_playing_with(d.path(), "Emerald", core.boxed());
    open_slots(&mut a, StateSlotMode::Save, 1);
    a.apply(Action::GbaDown(Btn::A));
    core.run_frame();
    let before = core.bytes();
    open_slots(&mut a, StateSlotMode::Load, 1);
    a.apply(Action::GbaDown(Btn::A));
    assert_ne!(core.bytes(), before);
    open_slots(&mut a, StateSlotMode::Load, 1);
    a.apply(Action::GbaDown(Btn::X));
    assert_eq!(core.bytes(), before);
}

#[test]
fn the_slot_picker_draws_its_title_four_slots_history_back_and_undo_hint() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    open_slots(&mut a, StateSlotMode::Load, 3);
    a.set_state_menu_faces(
        vec![
            (slot_ui::TexId::from_raw(10), 200, 36),
            (slot_ui::TexId::from_raw(11), 200, 36),
            (slot_ui::TexId::from_raw(12), 100, 36),
        ],
        vec![],
    );
    a.set_state_slot_faces(
        (20..26)
            .map(|tex| (slot_ui::TexId::from_raw(tex), 300, 36))
            .collect(),
    );
    a.set_state_slot_undo_face(Some((slot_ui::TexId::from_raw(30), 150, 24)));
    let mut draws = Vec::new();
    a.draw(&mut draws);
    let textures: Vec<_> = draws
        .iter()
        .filter_map(|draw| match draw {
            slot_ui::Draw::Tex { tex, y, h, .. } => Some((*tex, *y, *h)),
            _ => None,
        })
        .collect();
    assert_eq!(
        textures.iter().map(|(tex, _, _)| *tex).collect::<Vec<_>>(),
        [11, 20, 21, 22, 23, 24, 25, 30].map(slot_ui::TexId::from_raw)
    );
    for pair in textures.windows(2) {
        assert!(pair[0].1 + pair[0].2 <= pair[1].1);
    }
    assert!(
        draws.iter().any(
            |draw| matches!(draw, slot_ui::Draw::Rect { y, h, .. } if *y > 240.0 && *h == 36.0)
        ),
        "slot 4 was not highlighted"
    );
}

#[test]
fn slot_picker_back_returns_to_its_parent_and_menu_resumes_without_saving() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    for mode in [StateSlotMode::Save, StateSlotMode::Load] {
        open_slots(&mut a, mode, 0);
        a.apply(Action::GbaDown(Btn::Up));
        assert_eq!(a.state_slot_labels().last().unwrap(), "Back");
        assert_eq!(
            a.state_slot_menu().unwrap().1,
            a.state_slot_labels().len() - 1
        );
        a.apply(Action::GbaDown(Btn::A));
        assert_eq!(a.state_slot_menu(), None);
        assert!(a.game_menu_open());
        a.apply(Action::GbaDown(Btn::A));
        a.apply(Action::GbaDown(Btn::B));
        assert_eq!(a.state_slot_menu(), None);
        a.apply(Action::GbaDown(Btn::A));
        a.apply(Action::QuickMenu);
        assert!(!a.game_menu_open());
    }
    assert_eq!(states(d.path()), 0);
}

#[test]
fn failed_slot_write_keeps_the_picker_open_and_does_not_announce_a_save() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    std::fs::create_dir_all(ring.slot_path(1).unwrap()).unwrap();
    open_slots(&mut a, StateSlotMode::Save, 1);
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.state_slot_menu(), Some((StateSlotMode::Save, 1)));
    assert!(!a.undo_available(0));
    assert_ne!(a.toast(), Some(slot_ui::Toast::StateSaved));
}

#[test]
fn back_b_and_a_second_tap_close_it_without_saving() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    for close in [Action::GbaDown(Btn::B), Action::QuickMenu] {
        a.apply(Action::QuickMenu);
        a.apply(close);
        assert_eq!(a.state_menu_row(), None);
    }
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::Up));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.state_menu_row(), None);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(states(d.path()), 0);
}

#[test]
fn a_double_tap_still_reaches_the_saved_states_through_the_menu() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    assert_eq!(a.state_menu_row(), None);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
}

#[test]
fn a_held_menu_is_swallowed_while_open_and_ejects_once_closed() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    a.apply(Action::Eject);
    assert_eq!(a.state_menu_row(), Some(0));
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    a.apply(Action::GbaDown(Btn::B));
    a.apply(Action::Eject);
    assert_eq!(a.state_menu_row(), None);
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
}

fn open_on_run_ahead(a: &mut slot::app::App) {
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::Down));
    assert_eq!(a.state_menu_row(), Some(2));
}

#[test]
fn run_ahead_starts_off_and_steps_through_one_and_two_frames() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert_eq!(a.run_ahead(), 0);
    assert!(
        !d.path().join(RUN_AHEAD_FILE).exists(),
        "the fixture already wrote the file"
    );
    open_on_run_ahead(&mut a);
    for want in [1, 2, 0] {
        a.apply(Action::GbaDown(Btn::A));
        assert_eq!(a.run_ahead(), want);
        assert_eq!(a.state_menu_row(), Some(2), "changing it closed the menu");
    }
    a.apply(Action::GbaDown(Btn::Left));
    assert_eq!(a.run_ahead(), 2);
    a.apply(Action::GbaDown(Btn::Left));
    assert_eq!(a.run_ahead(), 1);
    a.apply(Action::GbaDown(Btn::Right));
    assert_eq!(a.run_ahead(), 2);
    assert_eq!(states(d.path()), 0);
}

#[test]
fn run_ahead_is_remembered_for_the_cart_being_played_only() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    open_on_run_ahead(&mut a);
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(run_ahead_for(d.path(), "Emerald"), 1);
    assert_eq!(
        run_ahead_for(d.path(), "Fusion"),
        0,
        "the choice was recorded against another cart as well"
    );
}

#[test]
fn left_and_right_leave_the_other_rows_alone() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::Left));
    assert_eq!(a.run_ahead(), 0);
    assert_eq!(a.state_menu_row(), Some(0));
}

#[test]
fn a_run_ahead_line_that_is_not_off_one_or_two_reads_as_off() {
    let d = tmp_root_with_carts(&["Emerald"]);
    for bad in [3, 200] {
        write_run_ahead(d.path(), "Emerald", bad).expect("write");
        assert_eq!(run_ahead_for(d.path(), "Emerald"), 0);
    }
    write_run_ahead(d.path(), "Emerald", 2).expect("write");
    assert_eq!(run_ahead_for(d.path(), "Emerald"), 2);
}

#[test]
fn the_players_row_opens_the_player_screen_over_the_held_game_and_back_returns() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert!(!a.choosing_players());
    a.apply(Action::QuickMenu);
    for _ in 0..3 {
        a.apply(Action::GbaDown(Btn::Down));
    }
    assert_eq!(a.state_menu_row(), Some(3));
    for back in [Btn::B, Btn::A] {
        a.apply(Action::GbaDown(Btn::A));
        assert!(a.choosing_players());
        assert!(
            a.game_menu_open(),
            "the game ran on behind the player screen"
        );
        a.apply(Action::GbaDown(Btn::Down));
        a.apply(Action::GbaDown(Btn::Left));
        assert!(a.choosing_players(), "a direction left the screen");
        assert_eq!(
            a.state_menu_row(),
            Some(3),
            "a direction moved the menu behind"
        );
        a.apply(Action::GbaDown(back));
        assert!(!a.choosing_players());
        assert_eq!(
            a.state_menu_row(),
            Some(3),
            "going back closed the whole menu"
        );
    }
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(states(d.path()), 0);
}

#[test]
fn reopening_the_menu_does_not_come_back_on_the_player_screen() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    for _ in 0..3 {
        a.apply(Action::GbaDown(Btn::Down));
    }
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::QuickMenu);
    a.apply(Action::QuickMenu);
    assert_eq!(a.state_menu_row(), None);
    a.apply(Action::QuickMenu);
    assert_eq!(a.state_menu_row(), Some(0));
    assert!(!a.choosing_players());
}
