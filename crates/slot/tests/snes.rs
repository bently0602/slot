#[path = "../../slot-retro/tests/common/snes.rs"]
mod homebrew;

use slot::app::{Phase, Playback, StateSlotMode};
use slot::audio::StubSink;
use slot::emu::Speed;
use slot::session::Session;
use slot_input::{Action, Btn, ControllerInfo, ControllerStatus, RawEvent};
use slot_store::{Core, Platform, SlotState, StateRing};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn core_path() -> Option<PathBuf> {
    let path = std::env::var_os("SLOT_TEST_SNES_CORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../target/desktop/snes9x_libretro.so"));
    if path.exists() {
        return Some(path);
    }
    assert!(
        std::env::var_os("SLOT_TEST_SNES_CORE").is_none(),
        "requested SNES core missing"
    );
    eprintln!("Snes9x unavailable; build it or set SLOT_TEST_SNES_CORE to run this test");
    None
}

fn step(session: &mut Session) {
    session.update(1.0 / 60.0);
    if matches!(session.app().phase(), Phase::Playing { .. })
        && (!session.app().game_menu_open() || session.app().playback().is_some())
    {
        if session.app().playback().is_some() {
            session.step_emulator(Duration::from_nanos(16_666_667), Duration::ZERO);
            std::thread::sleep(Duration::from_millis(20));
            return;
        }
        let until = Instant::now() + Duration::from_secs(2);
        while !session.step_emulator(Duration::from_nanos(16_666_667), Duration::from_secs(1)) {
            assert!(
                Instant::now() < until,
                "SNES worker did not accept a display tick"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn action(session: &mut Session, action: Action) {
    match action {
        Action::GbaDown(btn) => session.feed([RawEvent::Down(btn)], 5000),
        Action::GbaUp(btn) => session.feed([RawEvent::Up(btn)], 5000),
        _ => session.app_mut().apply(action),
    }
    step(session);
}

fn menu(session: &mut Session, row: usize) {
    action(session, Action::QuickMenu);
    for _ in 0..row {
        action(session, Action::GbaDown(Btn::Down));
    }
}

fn boot(root: &std::path::Path) -> Session {
    let mut session = Session::boot_with(root.to_path_buf(), Box::new(StubSink::draining()));
    session.set_driven(true);
    let until = Instant::now() + Duration::from_secs(10);
    while !matches!(session.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < until, "SNES session failed to start");
        session.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(2));
    }
    step(&mut session);
    session
}

fn ram(session: &mut Session, root: &std::path::Path) -> Vec<u8> {
    session.app_mut().flush_resume();
    slot::persist::read_sav(root, Platform::Snes, "Test").unwrap()
}

#[test]
fn snes_session_assigns_players_saves_loads_undoes_resumes_and_plays_back() {
    let Some(core) = core_path() else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    slot::root::ensure(root.path());
    std::fs::copy(
        core,
        root.path()
            .join("System")
            .join(slot::core::dylib_name(Core::Snes9x)),
    )
    .unwrap();
    for stem in ["Test", "Other"] {
        std::fs::write(
            root.path().join(format!("Games/SNES/{stem}.sfc")),
            homebrew::rom(false, false),
        )
        .unwrap();
    }
    slot_store::write_slot_state(
        root.path(),
        &SlotState {
            cart: Some("Test".into()),
            cart_platform: Some(Platform::Snes),
            clock_set: true,
            rewind: true,
            turbo: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut session = boot(root.path());
    assert_eq!(session.app().core(), Core::Snes9x);
    assert_eq!(session.app().picture_rect(), slot_gfx::WHOLE_TEXTURE);
    assert!(session.app().taken_buttons().is_empty());
    let ring = StateRing::new(root.path(), Platform::Snes, Core::Snes9x, "Test");

    // Move the handheld to player four and send three independent external pads.
    let status = ControllerStatus {
        connected: (1..=3)
            .map(|i| ControllerInfo {
                id: i.to_string(),
                name: i.to_string(),
            })
            .collect(),
        active: Some("1".into()),
        players: vec![1, 2, 3],
        handheld: 4,
    };
    session.update_controllers(status);
    session.set_player_buttons(&[vec![Btn::X], vec![Btn::Y], vec![Btn::A], vec![]]);
    action(&mut session, Action::GbaDown(Btn::R1));
    for _ in 0..10 {
        step(&mut session);
    }
    let before = ram(&mut session, root.path());
    for (player, expected) in [0x40, 0x4000, 0x80, 0x10].into_iter().enumerate() {
        assert_eq!(
            homebrew::buttons(&before, player),
            expected,
            "assigned player {}",
            player + 1
        );
    }

    // Real states in all four numbered slots, then load and undo a changed game.
    for slot in 0..4 {
        menu(&mut session, 0);
        action(&mut session, Action::GbaDown(Btn::A));
        assert_eq!(
            session.app().state_slot_menu(),
            Some((StateSlotMode::Save, 0))
        );
        for _ in 0..slot {
            action(&mut session, Action::GbaDown(Btn::Down));
        }
        action(&mut session, Action::GbaDown(Btn::A));
        assert!(ring.read_slot(slot).unwrap().unwrap().len() > 100_000);
    }
    let saved = ring.read_slot(0).unwrap().unwrap();
    action(&mut session, Action::GbaUp(Btn::R1));
    session.set_player_buttons(&[vec![], vec![], vec![], vec![]]);
    for _ in 0..10 {
        step(&mut session);
    }
    let changed = ram(&mut session, root.path());
    let count = |ram: &[u8]| u16::from_le_bytes(ram[64..66].try_into().unwrap());
    menu(&mut session, 1);
    action(&mut session, Action::GbaDown(Btn::A));
    action(&mut session, Action::GbaDown(Btn::A));
    // Loading runs again with released controls. Undo remains accessible in the picker.
    menu(&mut session, 1);
    assert!(
        count(&ram(&mut session, root.path())) < count(&changed),
        "load did not restore the saved timeline"
    );
    action(&mut session, Action::GbaDown(Btn::A));
    action(&mut session, Action::GbaDown(Btn::X));
    assert_eq!(ring.read_slot(0).unwrap().unwrap(), saved);
    assert_eq!(
        ram(&mut session, root.path()),
        changed,
        "undo did not restore the pre-load state"
    );
    action(&mut session, Action::GbaDown(Btn::B));
    if session.app().game_menu_open() {
        action(&mut session, Action::QuickMenu);
    }

    menu(&mut session, 2);
    action(&mut session, Action::GbaDown(Btn::Right));
    assert_eq!(session.app().run_ahead(), 1);
    action(&mut session, Action::QuickMenu);
    for _ in 0..20 {
        step(&mut session);
    }
    assert!(ram(&mut session, root.path()).len() >= 66);
    assert_eq!(slot::run_ahead::run_ahead_for(root.path(), "Test"), 1);

    // Rewind records the real core and reduces its monotonically increasing loop count.
    session.app_mut().set_run_ahead(0);
    for _ in 0..60 {
        step(&mut session);
    }
    let played = ram(&mut session, root.path());
    menu(&mut session, 4);
    action(&mut session, Action::GbaDown(Btn::A));
    assert_eq!(session.app().playback(), Some(Playback::Rewind));
    for _ in 0..24 {
        step(&mut session);
    }
    action(&mut session, Action::GbaUp(Btn::A));
    assert!(
        count(&ram(&mut session, root.path())) < count(&played),
        "SNES rewind did not go backwards"
    );
    action(&mut session, Action::GbaDown(Btn::B));
    menu(&mut session, 5);
    action(&mut session, Action::GbaDown(Btn::A));
    let until = Instant::now() + Duration::from_secs(2);
    while session.observed_speed() != Some(Speed::Fast) {
        assert!(Instant::now() < until, "SNES fast-forward did not start");
        step(&mut session);
    }
    assert_eq!(session.observed_speed(), Some(Speed::Fast));
    for _ in 0..5 {
        step(&mut session);
    }
    action(&mut session, Action::GbaUp(Btn::A));
    assert_eq!(session.app().playback(), None);
    action(&mut session, Action::GbaDown(Btn::B));

    // Resume stays on the SNES shelf and uses its own state/SRAM paths.
    let final_ram = ram(&mut session, root.path());
    assert!(ring.read_resume().unwrap().is_some());
    drop(session);
    let mut resumed = boot(root.path());
    assert!(matches!(resumed.app().phase(), Phase::Playing { .. }));
    assert_eq!(resumed.app().platform(), Platform::Snes);
    assert!(count(&ram(&mut resumed, root.path())) >= count(&final_ram));
}
