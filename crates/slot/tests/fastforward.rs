mod common;

use std::time::{Duration, Instant};

use slot::app::Phase;
use slot::session::Session;
use slot_input::{Btn, Millis, RawEvent};
use slot_store::{write_slot_state, SlotState};
use slot_ui::Icon;

#[test]
fn fast_forward_runs_only_while_a_is_held_on_its_menu_row() {
    use slot::emu::Speed;
    let d = common::tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut s = Session::boot_with(
        d.path().to_path_buf(),
        Box::new(slot::audio::StubSink::draining()),
    );
    let mut now = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline);
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::R2)));
    assert!(badge(&s).is_none(), "a trigger started fast forward");
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::Left)));
    assert_ne!(s.emu().unwrap().input().0, 0);
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::Menu)));
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::Menu)));
    assert_eq!(s.emu().unwrap().input().0, 0);
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::Left)));
    for _ in 0..5 {
        step(&mut s, &mut now, Some(RawEvent::Down(Btn::Down)));
        step(&mut s, &mut now, Some(RawEvent::Up(Btn::Down)));
    }
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::A)));
    assert_eq!(badge(&s), Some(Icon::FastForward));
    s.set_player_buttons(&std::array::from_fn(|_| vec![Btn::A, Btn::Left]));
    assert_eq!(
        s.emu().unwrap().input().0,
        0,
        "menu inputs reached the game"
    );
    s.set_player_buttons(&Default::default());
    wait_speed(&s, Speed::Fast);
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::A)));
    assert!(badge(&s).is_none());
    assert_eq!(s.app().state_menu_row(), Some(5));
    wait_speed(&s, Speed::Paused);
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::B)));
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::B)));
    wait_speed(&s, Speed::Normal);
    assert!(!s.app().game_menu_open());
}

fn wait_speed(s: &Session, speed: slot::emu::Speed) {
    let until = Instant::now() + Duration::from_secs(2);
    while s.observed_speed() != Some(speed) {
        assert!(Instant::now() < until, "worker did not enter {speed:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn docked_controller_reconnection_does_not_restart_menu_playback() {
    use slot::app::Playback;
    use slot::emu::Speed;
    use slot_input::{Action, ControllerInfo, ControllerMode, ControllerStatus};
    for (row, playback) in [(4, Playback::Rewind), (5, Playback::FastForward)] {
        let d = common::tmp_root_with_carts(&["Emerald"]);
        write_slot_state(
            d.path(),
            &SlotState {
                cart: Some("Emerald".into()),
                clock_set: true,
                ..Default::default()
            },
        )
        .unwrap();
        let mut s = Session::boot_with(
            d.path().to_path_buf(),
            Box::new(slot::audio::StubSink::draining()),
        );
        let mut now = 0;
        let deadline = Instant::now() + Duration::from_secs(10);
        while !matches!(s.app().phase(), Phase::Playing { .. }) {
            assert!(Instant::now() < deadline);
            step(&mut s, &mut now, None);
            std::thread::sleep(Duration::from_millis(1));
        }
        let connected = ControllerStatus {
            connected: vec![ControllerInfo {
                id: "ultimate".into(),
                name: "8BitDo Ultimate".into(),
            }],
            active: Some("ultimate".into()),
            players: vec![1],
            handheld: 1,
        };
        s.update_controllers(connected.clone());
        s.set_docked(true);
        s.app_mut().apply(Action::QuickMenu);
        for _ in 0..row {
            step(&mut s, &mut now, Some(RawEvent::Down(Btn::Down)));
            step(&mut s, &mut now, Some(RawEvent::Up(Btn::Down)));
        }
        step(&mut s, &mut now, Some(RawEvent::Down(Btn::A)));
        assert_eq!(s.app().playback(), Some(playback));
        assert_eq!(s.controller_mode(), ControllerMode::Menu);
        s.update_controllers(ControllerStatus::default());
        assert!(s.controller_paused());
        assert_eq!(s.app().playback(), None);
        wait_speed(&s, Speed::Paused);
        s.update_controllers(connected);
        assert!(!s.controller_paused());
        assert_eq!(s.app().playback(), None);
        assert_eq!(s.app().state_menu_row(), Some(row));
        wait_speed(&s, Speed::Paused);
        step(&mut s, &mut now, Some(RawEvent::Up(Btn::A)));
        step(&mut s, &mut now, Some(RawEvent::Down(Btn::A)));
        assert_eq!(s.app().playback(), Some(playback));
        step(&mut s, &mut now, Some(RawEvent::Up(Btn::A)));
        wait_speed(&s, Speed::Paused);
    }
}

#[test]
fn the_fast_forward_settings_reach_the_emulator() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            ff_speed: 2,
            ff_sound: true,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;

    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }

    let emu = s.emu().expect("a seated cart has a core");
    assert_eq!(
        emu.fast_steps(),
        2,
        "the chosen speed never reached the core"
    );
    assert!(emu.ff_sound(), "fast forward sound never reached the core");
}

fn step(s: &mut Session, now: &mut Millis, ev: Option<RawEvent>) {
    *now += 16;
    s.feed(ev, *now);
    s.update(1.0 / 60.0);
}

fn badge(s: &Session) -> Option<Icon> {
    s.app().ff_badge()
}
