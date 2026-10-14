mod common;

use common::{app_playing_in, tmp_root_with_carts};
use slot::app::{App, Phase};
use slot::persist::Snapshot;
use slot::system_app::{game_reset_command, reset_target, GameReset};
use slot_input::{Action, Btn};
use slot_store::{Core, Platform, StateRing};
use std::ffi::OsString;
use std::path::Path;

struct StuckSnapshot;
impl Snapshot for StuckSnapshot {
    fn state(&self) -> Option<Vec<u8>> {
        panic!("Reset asked the stuck core to save")
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        panic!("Reset asked for possibly damaged save RAM")
    }
    fn thumb(&self) -> Option<Vec<u8>> {
        panic!("Reset asked for a snapshot")
    }
    fn load(&self, _: Vec<u8>) {
        panic!("Reset loaded a saved state")
    }
}

#[test]
fn reset_is_one_shot_and_preserves_existing_saves_without_touching_the_core() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    a.apply(Action::SaveState);
    assert!(a.undo_available(0));
    assert_eq!(ring.list().unwrap().len(), 1);
    ring.write_slot(0, b"manual state").unwrap();
    ring.write_resume(b"broken resume").unwrap();
    slot::persist::write_sav(d.path(), Platform::Gba, "Emerald", b"last good save").unwrap();
    a.set_snapshot(Box::new(StuckSnapshot));
    a.apply(Action::QuickMenu);
    for _ in 0..6 {
        a.apply(Action::GbaDown(Btn::Down));
    }
    assert!(a.take_game_reset_request().is_none());
    a.apply(Action::GbaDown(Btn::A));
    let reset = a
        .take_game_reset_request()
        .expect("Reset Game was not dispatched");
    assert_eq!(reset.root, d.path());
    assert_eq!(reset.cart, "Emerald");
    assert_eq!(reset.platform, Platform::Gba);
    assert!(a.take_game_reset_request().is_none());
    assert!(!a.undo_available(0));
    assert_eq!(ring.list().unwrap().len(), 1);
    assert_eq!(ring.read_slot(0).unwrap().unwrap(), b"manual state");
    assert_eq!(ring.read_resume().unwrap().unwrap(), b"broken resume");
    assert_eq!(
        slot::persist::read_sav(d.path(), Platform::Gba, "Emerald").unwrap(),
        b"last good save"
    );
}

#[test]
fn a_failed_slot_save_can_back_out_and_reach_reset() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    std::fs::create_dir_all(ring.slot_path(0).unwrap()).unwrap();
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::A));
    assert!(a.state_slot_menu().is_some());
    a.apply(Action::GbaDown(Btn::B));
    for _ in 0..6 {
        a.apply(Action::GbaDown(Btn::Down));
    }
    a.apply(Action::GbaDown(Btn::A));
    assert!(a.take_game_reset_request().is_some());
}

#[test]
fn clean_start_selects_the_requested_cart_even_with_another_cart_remembered() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = App::boot(d.path());
    a.start_game_clean("Fusion", Platform::Gba);
    assert!(matches!(a.phase(), Phase::Inserting { cart, clean: true, .. } if cart == "Fusion"));
    assert!(a.starting_clean());
    a.start_game_clean("missing", Platform::Gba);
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn reset_relaunches_native_and_loader_invocations_and_preserves_spaces() {
    let reset = GameReset {
        root: "/card with spaces".into(),
        cart: "Game with spaces".into(),
        platform: Platform::Snes,
    };
    let args: Vec<OsString> = [
        "/card with spaces/System/slot",
        "--return-device-info",
        "--reset-game",
        "GBA",
        "old game",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    for executable in [
        "/card with spaces/System/slot",
        "/lib/ld-linux-aarch64.so.1",
    ] {
        let cmd = game_reset_command(Path::new(executable), &args, &reset);
        let actual: Vec<_> = cmd.get_args().map(|arg| arg.to_str().unwrap()).collect();
        let mut expected = vec!["--reset-game", "SNES", "Game with spaces"];
        if executable.starts_with("/lib/") {
            expected.insert(0, "/card with spaces/System/slot");
        }
        assert_eq!(actual, expected);
        assert_eq!(
            reset_target(&cmd.get_args().map(OsString::from).collect::<Vec<_>>()),
            Some(("Game with spaces".into(), Platform::Snes))
        );
        assert!(cmd
            .get_envs()
            .any(|(key, value)| key == "SLOT_ROOT" && value == Some(reset.root.as_os_str())));
    }
    assert!(reset_target(&["--reset-game".into(), "unknown".into(), "game".into()]).is_none());
    assert!(reset_target(&["--reset-game".into(), "GBA".into()]).is_none());
}
