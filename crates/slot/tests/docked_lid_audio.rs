mod common;

use common::{app_playing_in, boot, clocked, panel, tmp_root_with_carts, tmp_root_with_real_carts};
use slot::app::Phase;
use slot::audio::{playback_devices, AudioError, AudioRoute, AudioSink, Ring, StubSink};
use slot::emu::Speed;
use slot::session::Session;
use slot_input::{Action, Btn, ControllerInfo, ControllerStatus, RawEvent};
use slot_store::{read_slot_state, write_slot_state, AudioOutput, SlotState};
use std::sync::{atomic::Ordering, Arc, Mutex};
use std::time::{Duration, Instant};

#[test]
fn a_docked_closed_lid_keeps_the_game_running_and_the_panel_dark() {
    let dir = tmp_root_with_carts(&["Emerald"]);
    let mut app = app_playing_in(dir.path(), "Emerald");
    let (power, light) = panel(dir.path(), Duration::from_secs(2));
    app.set_power(power);
    app.set_docked(true);
    app.apply(Action::LidClose);
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    assert_eq!(light.load(Ordering::Relaxed), 0);
    app.apply(Action::BrightnessUp);
    assert_eq!(light.load(Ordering::Relaxed), 0);
    for _ in 0..180 {
        app.update(1.0 / 60.0);
    }
    assert!(!app.powering_off());
    assert!(app.checkpoint_for_display().unwrap());
    app.apply(Action::LidOpen);
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    assert_eq!(light.load(Ordering::Relaxed), app.brightness());
}

#[test]
fn closed_lid_undocking_sleeps_and_redocking_only_wakes_lid_sleep() {
    let dir = tmp_root_with_carts(&["Emerald"]);
    let mut app = app_playing_in(dir.path(), "Emerald");
    let (power, light) = panel(dir.path(), Duration::from_secs(60));
    app.set_power(power);
    app.set_docked(true);
    app.apply(Action::LidClose);
    app.set_docked(false);
    assert!(matches!(app.phase(), Phase::Doze { cart: Some(_) }));
    app.set_docked(true);
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    assert_eq!(light.load(Ordering::Relaxed), 0);
    app.apply(Action::PowerTap);
    assert!(matches!(app.phase(), Phase::Doze { .. }));
    app.set_docked(false);
    app.set_docked(true);
    app.apply(Action::LidOpen);
    assert!(
        matches!(app.phase(), Phase::Doze { .. }),
        "opening the lid must not undo manual docked sleep"
    );
    assert_eq!(light.load(Ordering::Relaxed), 0);
    app.apply(Action::PowerTap);
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    assert_eq!(light.load(Ordering::Relaxed), app.brightness());
}

#[test]
fn disabling_the_exception_preserves_handheld_lid_sleep() {
    let dir = tmp_root_with_carts(&["Emerald"]);
    clocked(dir.path());
    let mut state = read_slot_state(dir.path());
    state.docked_lid = false;
    state.cart = Some("Emerald".into());
    write_slot_state(dir.path(), &state).unwrap();
    let mut app = boot(dir.path());
    app.set_snapshot(common::StubSnapshot::boxed());
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    app.set_docked(true);
    app.apply(Action::LidClose);
    assert!(matches!(app.phase(), Phase::Doze { .. }));
    app.apply(Action::LidOpen);
    assert!(matches!(app.phase(), Phase::Playing { .. }));
}

#[test]
fn a_closed_lid_restart_preserves_the_pending_saved_cart_load() {
    let dir = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(dir.path());
    let mut state = read_slot_state(dir.path());
    state.cart = Some("Emerald".into());
    write_slot_state(dir.path(), &state).unwrap();
    let mut app = boot(dir.path());
    assert!(matches!(app.phase(), Phase::Inserting { .. }));
    app.apply(Action::LidClose);
    assert!(matches!(app.phase(), Phase::Doze { cart: Some(c) } if c == "Emerald"));
    app.apply(Action::LidOpen);
    assert!(matches!(app.phase(), Phase::Inserting { cart, .. } if cart == "Emerald"));
    app.on_core_ready();
    app.update(1.0 / 60.0);
    assert!(matches!(app.phase(), Phase::Playing { cart } if cart == "Emerald"));
}

fn wait_speed(session: &Session, expected: Speed) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while session.observed_speed() != Some(expected) {
        assert!(Instant::now() < deadline, "core did not reach {expected:?}");
        session.step_emulator(Duration::from_millis(16), Duration::from_millis(20));
    }
}

#[test]
fn a_real_session_keeps_producing_tv_frames_with_the_lid_closed_and_stops_on_undock() {
    let _lock = common::core_lock();
    let dir = tmp_root_with_real_carts(&["Emerald", "Fusion"]);
    clocked(dir.path());
    let mut session = Session::boot_with(dir.path().to_path_buf(), Box::new(StubSink::draining()));
    session.feed([RawEvent::Down(Btn::A), RawEvent::Up(Btn::A)], 10);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(session.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline);
        session.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    session.set_driven(true);
    session.set_docked(true);
    session.update_controllers(ControllerStatus {
        connected: vec![ControllerInfo {
            id: "Ultimate".into(),
            name: "8BitDo Ultimate".into(),
        }],
        active: Some("Ultimate".into()),
    });
    session.feed([RawEvent::Down(Btn::Right), RawEvent::Down(Btn::Lid)], 100);
    wait_speed(&session, Speed::Normal);
    let before = session.frames_published();
    for _ in 0..3 {
        session.step_emulator(Duration::from_millis(16), Duration::from_millis(50));
    }
    assert!(session.frames_published() > before);
    session.set_docked(false);
    wait_speed(&session, Speed::Paused);
    assert_eq!(session.emu().unwrap().input().0, 0);
    let stopped = session.frames_published();
    for _ in 0..3 {
        session.step_emulator(Duration::from_millis(16), Duration::from_millis(20));
    }
    assert_eq!(session.frames_published(), stopped);
    session.feed([RawEvent::Up(Btn::Lid)], 200);
    wait_speed(&session, Speed::Normal);
}

const CARDS: &str = " 2 [sndhdmi ]: sndhdmi - HDMI Audio\n                      sunxi HDMI\n 0 [sndcodec ]: sndcodec - sunxi-codec\n 1 [USB ]: USB-Audio - USB Audio CODEC\n";
const PCMS: &str = "02-00: pcm0 : playback 1\n00-00: sunxi-CODEC : playback 1 : capture 1\n01-00: USB Audio : playback 1\n00-01: mic : capture 1\n02-02: HDMI 2 : playback 1\n";

#[test]
fn auto_audio_finds_hdmi_by_name_then_the_codec_and_avoids_usb_sound_cards() {
    let route = AudioRoute {
        output: AudioOutput::Auto,
        docked: true,
    };
    assert_eq!(
        playback_devices(CARDS, PCMS, route),
        [
            "plughw:2,0",
            "plughw:2,2",
            "plug:default",
            "default",
            "plughw:0,0"
        ]
    );
    assert_eq!(
        playback_devices(
            CARDS,
            PCMS,
            AudioRoute {
                docked: false,
                ..route
            }
        ),
        ["plug:default", "default", "plughw:0,0"]
    );
    assert_eq!(
        playback_devices(
            CARDS,
            PCMS,
            AudioRoute {
                output: AudioOutput::Device,
                ..route
            }
        ),
        ["plug:default", "default", "plughw:0,0"]
    );
    assert_eq!(
        playback_devices(
            CARDS,
            PCMS,
            AudioRoute {
                output: AudioOutput::Hdmi,
                docked: false
            }
        ),
        ["plug:default", "default", "plughw:0,0"]
    );
    let reversed = "00-00: HDMI : playback 1\n03-00: sunxi-codec : playback 1\n";
    assert_eq!(
        playback_devices("", reversed, route),
        ["plughw:0,0", "plug:default", "default", "plughw:3,0"]
    );
}

#[test]
fn audio_discovery_handles_missing_or_malformed_inventory_without_mistaking_capture_for_output() {
    let route = AudioRoute::default();
    assert_eq!(
        playback_devices(
            "",
            "bad\n00-00: mic : capture 1\nx-1: codec : playback 1",
            route
        ),
        ["plug:default", "default", "plughw:0,0", "hw:0,0"]
    );
    assert!(
        playback_devices(CARDS, "02-00: HDMI : playback 1", route).is_empty(),
        "Device fallback must not open a known HDMI card"
    );
}

#[test]
fn tv_audio_uses_device_volume_and_leaves_the_headphone_mute_and_volume_intact() {
    let dir = tmp_root_with_carts(&["Emerald"]);
    let (mut app, jack) = common::app_playing_with_jack(dir.path(), "Emerald");
    app.apply(Action::VolumeDown);
    let speaker = app.volume();
    jack.store(true, Ordering::Relaxed);
    app.tick_ms(10000);
    let headphones = app.volume();
    assert_ne!(speaker, headphones);
    app.apply(Action::MuteToggle);
    assert!(app.muted());
    app.set_hdmi_audio(true);
    assert!(!app.muted());
    assert_eq!(app.output_volume(), speaker);
    app.apply(Action::VolumeUp);
    app.set_hdmi_audio(false);
    assert!(app.muted());
    assert_eq!(app.volume(), headphones);
    app.flush_state();
    let state = read_slot_state(dir.path());
    assert!(state.muted_hp);
    assert!(!state.muted);
    assert_eq!(state.volume_hp, headphones);
}

struct RoutingSink {
    ring: Arc<Ring>,
    routes: Arc<Mutex<Vec<AudioRoute>>>,
}
impl AudioSink for RoutingSink {
    fn open(&mut self, _: u32) -> Result<(), AudioError> {
        Ok(())
    }
    fn ring(&self) -> Arc<Ring> {
        self.ring.clone()
    }
    fn configure_route(&mut self, route: AudioRoute) {
        self.routes.lock().unwrap().push(route);
    }
}

#[test]
fn session_routing_tracks_the_saved_setting_and_dock_state_without_replacing_the_audio_ring() {
    let dir = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let state = SlotState {
        audio_output: AudioOutput::Device,
        clock_set: true,
        ..SlotState::default()
    };
    write_slot_state(dir.path(), &state).unwrap();
    let ring = Arc::new(Ring::new(1024));
    let routes = Arc::new(Mutex::new(Vec::new()));
    let mut session = Session::boot_with(
        dir.path().to_path_buf(),
        Box::new(RoutingSink {
            ring: ring.clone(),
            routes: routes.clone(),
        }),
    );
    session.set_docked(true);
    assert_eq!(
        routes.lock().unwrap().last().copied(),
        Some(AudioRoute {
            output: AudioOutput::Device,
            docked: true
        })
    );
    session.set_docked(false);
    session.update(1.0 / 60.0);
    assert_eq!(
        routes.lock().unwrap().last().copied(),
        Some(AudioRoute {
            output: AudioOutput::Device,
            docked: false
        })
    );
    assert!(Arc::ptr_eq(&ring, &session.audio_ring()));
}
