mod common;
use common::{boot, clocked, tmp_root_with_carts, tmp_root_with_real_carts};
use slot::app::{App, Phase};
use slot::audio::StubSink;
use slot::emu::Speed;
use slot::input::controller::{AxisRange, Controller};
use slot::input::controller_hub::ControllerHub;
use slot::input::evdev::{Ev, EV_ABS};
use slot::session::Session;
use slot_input::{
    Action, Btn, ControllerConfig, ControllerInfo, ControllerMode, ControllerProfile,
    ControllerStatus, RawEvent,
};
use slot_store::{read_controller_config, write_controller_config};
use slot_ui::{QuickRow, QuickValue};
use std::time::{Duration, Instant};

fn info(id: &str) -> ControllerInfo {
    ControllerInfo {
        id: id.into(),
        name: format!("8BitDo {id}"),
    }
}
fn status(id: Option<&str>) -> ControllerStatus {
    ControllerStatus {
        connected: id.map(info).into_iter().collect(),
        active: id.map(str::to_owned),
        players: id.map(|_| 1).into_iter().collect(),
        handheld: 1,
    }
}

#[test]
fn auto_waits_for_the_original_pad_instead_of_giving_control_to_another_pad() {
    let mut hub = ControllerHub::default();
    hub.connect(1, info("Ultimate"));
    hub.connect(2, info("Other"));
    hub.set_mode(ControllerMode::Game);
    assert_eq!(hub.status().active.as_deref(), Some("Ultimate"));
    assert!(hub.set(2, vec![Btn::A]).is_empty());
    assert_eq!(hub.set(1, vec![Btn::B]), vec![RawEvent::Down(Btn::B)]);
    assert_eq!(hub.disconnect(1), vec![RawEvent::Up(Btn::B)]);
    assert_eq!(hub.status().active, None);
    assert!(hub.set(2, vec![Btn::X]).is_empty());
    hub.connect(3, info("Ultimate"));
    assert_eq!(hub.status().active.as_deref(), Some("Ultimate"));
    assert_eq!(hub.set(3, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);
}

#[test]
fn two_identical_pads_do_not_take_over_each_others_disconnect() {
    let mut hub = ControllerHub::default();
    hub.connect(1, info("Ultimate"));
    hub.connect(2, info("Ultimate"));
    hub.set_mode(ControllerMode::Game);
    hub.set(2, vec![Btn::A]);
    hub.disconnect(1);
    assert_eq!(hub.status().active, None);
    assert!(hub.set(2, vec![Btn::B]).is_empty());
    hub.connect(3, info("Ultimate"));
    assert_eq!(hub.status().active.as_deref(), Some("Ultimate"));
    assert_eq!(hub.set(3, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);
}

#[test]
fn profiles_and_pad_selection_release_old_buttons_and_wait_for_a_fresh_press() {
    let mut hub = ControllerHub::default();
    hub.connect(1, info("Ultimate"));
    hub.connect(2, info("Other"));
    hub.set_mode(ControllerMode::Game);
    assert_eq!(hub.set(1, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);
    let mut config = ControllerConfig {
        profiles: vec![ControllerProfile {
            id: "Ultimate".into(),
            swap_ab: true,
            swap_xy: true,
            deadzone: 40,
            ..ControllerProfile::default()
        }],
        ..ControllerConfig::default()
    };
    assert_eq!(hub.configure(&config), vec![RawEvent::Up(Btn::A)]);
    assert!(hub.set(1, vec![Btn::A]).is_empty());
    assert!(hub.set(1, vec![]).is_empty());
    assert_eq!(
        hub.set(1, vec![Btn::A, Btn::X]),
        vec![RawEvent::Down(Btn::B), RawEvent::Down(Btn::Y)]
    );
    assert_eq!(hub.deadzone(1), 40);
    assert_eq!(hub.deadzone(2), 25);
    hub.set(2, vec![Btn::A]);
    config.preferred = Some("Other".into());
    assert_eq!(
        hub.configure(&config),
        vec![RawEvent::Up(Btn::B), RawEvent::Up(Btn::Y)]
    );
    assert!(hub.set(2, vec![Btn::A]).is_empty());
    hub.set(2, vec![]);
    assert_eq!(hub.set(2, vec![Btn::A]), vec![RawEvent::Down(Btn::A)]);
}

#[test]
fn deadzone_changes_the_usable_stick_range_without_changing_the_hat() {
    let mut controller = Controller::default();
    controller.add_axis(
        0,
        AxisRange {
            min: -1000,
            max: 1000,
            flat: 0,
        },
        None,
    );
    controller.set_deadzone(40);
    assert!(controller
        .feed(Ev {
            kind: EV_ABS,
            code: 0,
            value: 300
        })
        .is_empty());
    assert_eq!(
        controller.feed(Ev {
            kind: EV_ABS,
            code: 0,
            value: 500
        }),
        vec![RawEvent::Down(Btn::Right)]
    );
    assert_eq!(
        controller.feed(Ev {
            kind: EV_ABS,
            code: 0,
            value: 300
        }),
        vec![RawEvent::Up(Btn::Right)]
    );
    assert_eq!(
        controller.feed(Ev {
            kind: EV_ABS,
            code: 0x10,
            value: 1
        }),
        vec![RawEvent::Down(Btn::Right)]
    );
}

fn open(app: &mut App, row: QuickRow) {
    app.apply(Action::QuickMenu);
    for _ in 0..QuickRow::Controllers.position() {
        app.apply(Action::GbaDown(Btn::Down));
    }
    app.apply(Action::GbaDown(Btn::A));
    for _ in 0..row.position() {
        app.apply(Action::GbaDown(Btn::Down));
    }
    assert_eq!(app.quick_menu(), Some(row));
}

#[test]
fn controller_menu_saves_individual_profiles_and_can_reset_the_selected_profile() {
    let root = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut app = boot(root.path());
    app.set_controller_status(status(Some("Ultimate")));
    open(&mut app, QuickRow::ControllerSwapAb);
    app.apply(Action::GbaDown(Btn::Right));
    assert_eq!(
        app.quick_value(QuickRow::ControllerSwapAb),
        Some(QuickValue::On)
    );
    assert!(
        read_controller_config(root.path())
            .profile("Ultimate")
            .swap_ab
    );
    app.apply(Action::GbaDown(Btn::Down));
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::Down));
    app.apply(Action::GbaDown(Btn::Right));
    assert_eq!(
        read_controller_config(root.path())
            .profile("Ultimate")
            .deadzone,
        30
    );
    let mut reloaded = boot(root.path());
    reloaded.set_controller_status(status(Some("Ultimate")));
    assert_eq!(
        reloaded.quick_value(QuickRow::ControllerSwapXy),
        Some(QuickValue::On)
    );
    open(&mut reloaded, QuickRow::ControllerReset);
    reloaded.apply(Action::GbaDown(Btn::A));
    let profile = read_controller_config(root.path()).profile("Ultimate");
    assert!(!profile.swap_ab && !profile.swap_xy);
    assert_eq!(profile.deadzone, 25);
}

#[test]
fn selecting_a_pad_remembers_its_name_even_when_it_is_disconnected() {
    let root = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut app = boot(root.path());
    app.set_controller_status(status(Some("Ultimate")));
    open(&mut app, QuickRow::ControllerSelect);
    app.apply(Action::GbaDown(Btn::Right));
    assert_eq!(
        read_controller_config(root.path()).preferred.as_deref(),
        Some("Ultimate")
    );
    let app = boot(root.path());
    assert!(app.controller_text().contains("Ultimate"));
    assert!(app.controller_text().contains("lost"));
}

fn playing(root: &std::path::Path) -> Session {
    clocked(root);
    let mut session = Session::boot_with(root.to_path_buf(), Box::new(StubSink::draining()));
    session.feed([RawEvent::Down(Btn::A), RawEvent::Up(Btn::A)], 10);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(session.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "game did not start");
        session.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    session
}

fn wait_speed(session: &Session, speed: Speed) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while session.observed_speed() != Some(speed) {
        assert!(Instant::now() < deadline, "core never reached {speed:?}");
        session.step_emulator(Duration::from_millis(16), Duration::from_millis(20));
    }
}

#[test]
fn losing_the_docked_pad_pauses_the_core_and_clears_buttons_then_reconnecting_resumes() {
    let _lock = common::core_lock();
    let root = tmp_root_with_real_carts(&["Emerald", "Fusion"]);
    let mut session = playing(root.path());
    session.set_driven(true);
    session.set_docked(true);
    session.update_controllers(status(Some("Ultimate")));
    session.feed([RawEvent::Down(Btn::Right)], 100);
    assert_ne!(session.emu().unwrap().input().0, 0);
    session.update_controllers(status(None));
    assert!(session.controller_paused());
    assert_eq!(session.emu().unwrap().input().0, 0);
    wait_speed(&session, Speed::Paused);
    let frames = session.frames_published();
    for _ in 0..3 {
        session.step_emulator(Duration::from_millis(16), Duration::from_millis(20));
    }
    assert_eq!(
        session.frames_published(),
        frames,
        "the paused core kept advancing the game"
    );
    session.feed([RawEvent::Down(Btn::B)], 120);
    assert_eq!(session.emu().unwrap().input().0, 0);
    session.update_controllers(status(Some("Ultimate")));
    assert!(!session.controller_paused());
    wait_speed(&session, Speed::Normal);
    assert!(matches!(session.app().phase(), Phase::Playing { cart } if cart == "Emerald"));
    session.update_controllers(status(None));
    assert!(session.controller_paused());
    session.feed([RawEvent::Down(Btn::Start)], 160);
    assert!(!session.controller_paused());
    assert_eq!(
        session.emu().unwrap().input().0,
        0,
        "resume must not press Start inside the game"
    );
    session.update_controllers(status(None));
    assert!(!session.controller_paused(), "handheld override was lost");
    session.update_controllers(status(Some("Ultimate")));
    session.update_controllers(status(None));
    assert!(
        session.controller_paused(),
        "next disconnect must pause again"
    );
    session.set_docked(false);
    assert!(!session.controller_paused());
    wait_speed(&session, Speed::Normal);
}

#[test]
fn starting_docked_without_a_pad_loads_the_game_and_waits_for_handheld_start() {
    let _lock = common::core_lock();
    let root = tmp_root_with_real_carts(&["Emerald", "Fusion"]);
    clocked(root.path());
    let mut session = Session::boot_with(root.path().to_path_buf(), Box::new(StubSink::draining()));
    session.set_docked(true);
    session.feed([RawEvent::Down(Btn::A), RawEvent::Up(Btn::A)], 10);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(session.app().phase(), Phase::Playing { .. }) {
        assert!(
            Instant::now() < deadline,
            "docked boot never seated its game"
        );
        session.update_controllers(status(None));
        session.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    session.update_controllers(status(None));
    assert!(session.controller_paused());
    wait_speed(&session, Speed::Paused);
    session.feed([RawEvent::Down(Btn::Start)], 150);
    assert!(!session.controller_paused());
    wait_speed(&session, Speed::Normal);
}

#[test]
fn disabling_disconnect_pause_keeps_a_docked_game_running_without_a_pad() {
    let _lock = common::core_lock();
    let root = tmp_root_with_real_carts(&["Emerald", "Fusion"]);
    write_controller_config(
        root.path(),
        &ControllerConfig {
            pause_disconnect: false,
            ..ControllerConfig::default()
        },
    )
    .unwrap();
    let mut session = playing(root.path());
    session.set_docked(true);
    session.update_controllers(status(None));
    assert!(!session.controller_paused());
    wait_speed(&session, Speed::Normal);
}

#[test]
fn the_players_row_opens_the_player_screen_and_back_returns_to_it() {
    let root = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut app = boot(root.path());
    open(&mut app, QuickRow::ControllerPlayers);
    for back in [Btn::B, Btn::A] {
        app.apply(Action::GbaDown(Btn::A));
        assert!(matches!(app.phase(), slot::app::Phase::Players));
        app.apply(Action::GbaDown(Btn::Left));
        app.apply(Action::GbaDown(Btn::Down));
        assert!(
            matches!(app.phase(), slot::app::Phase::Players),
            "a direction left the screen"
        );
        app.apply(Action::GbaDown(back));
        assert_eq!(app.quick_menu(), Some(QuickRow::ControllerPlayers));
    }
}

#[test]
fn the_player_screen_lists_each_controller_with_its_player() {
    let root = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut app = boot(root.path());
    app.set_controller_status(ControllerStatus {
        connected: vec![info("One"), info("Two")],
        active: Some("One".into()),
        players: vec![2, 1],
        handheld: 1,
    });
    assert_eq!(
        app.controller_seats(),
        [
            ("Handheld".to_string(), 1),
            ("8BitDo One".to_string(), 2),
            ("8BitDo Two".to_string(), 1)
        ]
    );
    assert!(!app.multitap_wanted());
    app.set_controller_status(ControllerStatus {
        connected: vec![info("One"), info("Two")],
        active: Some("One".into()),
        players: vec![1, 3],
        handheld: 1,
    });
    assert!(app.multitap_wanted(), "player three needs the multitap");
    app.set_controller_status(ControllerStatus {
        handheld: 4,
        ..ControllerStatus::default()
    });
    assert_eq!(app.controller_seats(), [("Handheld".to_string(), 4)]);
    assert!(
        app.multitap_wanted(),
        "the handheld as player four needs it too"
    );
}

#[test]
fn controllers_drive_menus_on_the_shelf_and_choose_players_on_the_player_screen() {
    let root = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(root.path());
    let mut session = slot::session::Session::boot(root.path().to_path_buf());
    assert_eq!(session.controller_mode(), ControllerMode::Menu);
    open(session.app_mut(), QuickRow::ControllerPlayers);
    session.app_mut().apply(Action::GbaDown(Btn::A));
    assert_eq!(session.controller_mode(), ControllerMode::Assign);
    session.app_mut().apply(Action::GbaDown(Btn::B));
    assert_eq!(session.controller_mode(), ControllerMode::Menu);
}
