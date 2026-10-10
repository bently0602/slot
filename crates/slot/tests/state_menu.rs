mod common;

use common::{app_playing_in, tmp_root_with_carts};
use slot::app::Phase;
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
    assert_eq!(a.state_menu_row(), Some(2));
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::Down));
    assert_eq!(a.state_menu_row(), Some(1));
}

#[test]
fn save_state_pushes_a_state_each_time_and_closes_the_menu() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    for saved in 1..=2 {
        a.apply(Action::QuickMenu);
        a.apply(Action::GbaDown(Btn::A));
        assert_eq!(a.state_menu_row(), None);
        assert!(!a.game_menu_open());
        assert_eq!(states(d.path()), saved);
    }
}

#[test]
fn load_state_opens_the_saved_states_but_stays_put_when_there_are_none() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.state_menu_row(), Some(1), "nothing to load yet");
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    a.apply(Action::GbaDown(Btn::Up));
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.state_menu_row(), None);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    assert_eq!(a.polaroid_entries().len(), 1);
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
