mod common;
use common::{app_playing_in, boot, tmp_root_with_carts, StubSnapshot};
use slot::app::Phase;
use slot::persist::Snapshot;
use slot_input::Action;
use slot_store::{read_slot_state, Core, Platform, StateRing};

#[test]
fn docking_writes_a_resume_and_keeps_the_cart_selected_for_graphics_restart() {
    let dir = tmp_root_with_carts(&["Emerald"]);
    let mut app = app_playing_in(dir.path(), "Emerald");
    assert!(app.checkpoint_for_display().unwrap());
    let resume = StateRing::new(dir.path(), Platform::Gba, Core::Mgba, "Emerald")
        .read_resume()
        .unwrap()
        .unwrap();
    assert_eq!(resume, vec![9u8; 1024]);
    assert_eq!(read_slot_state(dir.path()).cart, Some("Emerald".into()));
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    let rebooted = boot(dir.path());
    assert_eq!(rebooted.seated_cart().unwrap().stem, "Emerald");
}

struct MissingSnapshot;
impl Snapshot for MissingSnapshot {
    fn state(&self) -> Option<Vec<u8>> {
        None
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        None
    }
    fn thumb(&self) -> Option<Vec<u8>> {
        None
    }
    fn load(&self, _: Vec<u8>) {}
}

#[test]
fn docking_defers_on_a_missing_snapshot_or_failed_write_and_preserves_the_running_game() {
    let dir = tmp_root_with_carts(&["Emerald"]);
    let mut app = app_playing_in(dir.path(), "Emerald");
    app.set_snapshot(Box::new(MissingSnapshot));
    assert!(app.checkpoint_for_display().is_err());
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    app.set_snapshot(StubSnapshot::boxed());
    app.flush_state();
    std::fs::remove_file(dir.path().join("Config/slot.state")).unwrap();
    std::fs::create_dir(dir.path().join("Config/slot.state")).unwrap();
    assert!(app.checkpoint_for_display().is_err());
    assert!(matches!(app.phase(), Phase::Playing { .. }));
}

#[test]
fn docking_waits_for_sleep_and_insertion_to_end() {
    let dir = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut app = boot(dir.path());
    app.apply(Action::Insert);
    assert!(!app.checkpoint_for_display().unwrap());
    let mut playing = app_playing_in(dir.path(), "Emerald");
    playing.apply(Action::LidClose);
    assert!(!playing.checkpoint_for_display().unwrap());
    playing.apply(Action::LidOpen);
    assert!(playing.checkpoint_for_display().unwrap());
}
