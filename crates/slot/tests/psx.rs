use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{Cart, Platform};

fn animation_app() -> App {
    let carts = ["Adventure", "Second"].map(|stem| Cart {
        platform: Platform::Psx,
        stem: stem.into(),
        rom: format!("{stem}.chd").into(),
        label: None,
        title: stem.into(),
        code: String::new(),
        shell: None,
    });
    let mut app = App::new(carts.to_vec());
    app.set_platform(Platform::Psx);
    app
}

#[test]
fn psx_waits_for_the_case_and_disc_animation_and_plays_the_insert_sound_at_seating() {
    let mut app = animation_app();
    app.apply(Action::Insert);
    app.on_core_ready();
    app.update(slot::app::INSERT_S);
    assert!(matches!(app.phase(), Phase::Inserting { .. }));
    assert!(
        app.seat() < 0.54,
        "disc should still be lifting out of the case"
    );
    assert_eq!(app.take_sfx(), None);
    app.update(slot::app::PSX_SEATED_AT - slot::app::INSERT_S);
    assert_eq!(app.seat(), 1.0);
    assert_eq!(app.take_sfx(), Some(slot::audio::Sfx::Insert));
    assert!(matches!(app.phase(), Phase::Inserting { .. }));
    app.update(0.29);
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    app.apply(Action::Eject);
    for _ in 0..130 {
        app.update(1.0 / 60.0);
    }
    assert!(matches!(app.phase(), Phase::Shelf));
}

#[test]
fn a_failed_psx_launch_reverses_from_its_current_animation_pose() {
    for progress in [0.15, 0.45, 0.8] {
        let mut app = animation_app();
        app.apply(Action::Insert);
        app.update(slot::app::PSX_SEATED_AT * progress);
        let seat = app.seat();
        app.on_core_failed();
        assert!((app.seat() - seat).abs() < 0.001);
        app.update(slot::app::PSX_SEATED_AT * progress + 0.01);
        assert!(matches!(app.phase(), Phase::Shelf));
    }
}

#[test]
fn a_seated_psx_game_resumes_without_waiting_for_the_case_animation() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("Config")).unwrap();
    std::fs::create_dir_all(root.path().join("Games/PSX")).unwrap();
    for stem in ["Adventure", "Second"] {
        std::fs::write(root.path().join(format!("Games/PSX/{stem}.chd")), b"disc").unwrap();
    }
    slot_store::write_slot_state(
        root.path(),
        &slot_store::SlotState {
            clock_set: true,
            cart: Some("Adventure".into()),
            cart_platform: Some(Platform::Psx),
            ..Default::default()
        },
    )
    .unwrap();
    let mut app = App::boot(root.path());
    app.set_platform(Platform::Psx);
    app.on_core_ready();
    app.update(1.0 / 60.0);
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    assert_eq!(app.take_sfx(), None);
}

#[test]
fn ejecting_during_a_psx_launch_reverses_without_a_jump() {
    let mut app = animation_app();
    app.apply(Action::Insert);
    app.update(0.6);
    let seat = app.seat();
    app.apply(Action::Eject);
    assert!((app.seat() - seat).abs() < 0.001);
    app.update(0.2);
    assert!(app.seat() < seat);
    app.update(0.41);
    assert!(matches!(app.phase(), Phase::Shelf));
}

struct DigitalCore {
    mock: slot_retro::MockCore,
    inputs: std::sync::Arc<std::sync::Mutex<Vec<slot_retro::ButtonMask>>>,
    disc: usize,
    state_bytes: usize,
}
impl slot_retro::RetroCore for DigitalCore {
    fn discs(&self) -> (usize, usize) {
        (self.disc, 2)
    }
    fn change_disc(&mut self, index: usize) -> Result<(), slot_retro::CoreError> {
        self.disc = index;
        Ok(())
    }
    fn full_joypad(&self) -> bool {
        true
    }
    fn load(&mut self, rom: &std::path::Path) -> Result<(), slot_retro::CoreError> {
        self.mock.load(rom)
    }
    fn run_frame(&mut self, input: slot_retro::ButtonMask) {
        self.inputs.lock().unwrap().push(input);
        self.mock.run_frame(input);
    }
    fn video_xrgb8888(&self) -> &[u8] {
        self.mock.video_xrgb8888()
    }
    fn take_audio(&mut self) -> Vec<i16> {
        let _ = self.mock.take_audio();
        Vec::new()
    }
    fn serialize(&mut self) -> Result<Vec<u8>, slot_retro::CoreError> {
        let mut state = self.mock.serialize()?;
        state.push(self.disc as u8);
        // Exercise multi-megabyte states, as used by the PS1 core.
        state.resize(self.state_bytes, 0);
        Ok(state)
    }
    fn unserialize(&mut self, data: &[u8]) -> Result<(), slot_retro::CoreError> {
        self.disc = usize::from(data[8]);
        self.mock.unserialize(&data[..8])
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        self.mock.save_ram()
    }
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), slot_retro::CoreError> {
        self.mock.load_save_ram(data)
    }
    fn av_info(&self) -> slot_retro::AvInfo {
        slot_retro::AvInfo {
            fps: 50.0,
            sample_rate: 44100.0,
        }
    }
}

#[test]
fn pal_timing_and_all_digital_buttons_reach_the_worker() {
    use slot::audio::{AudioSink, StubSink};
    use slot::emu::{EmuHandle, Speed};
    use slot_retro::ButtonMask;
    use std::time::{Duration, Instant};
    let inputs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut sink = StubSink::new();
    sink.open(44100).unwrap();
    let emu = EmuHandle::spawn(
        Box::new(DigitalCore {
            mock: slot_retro::MockCore::new(),
            inputs: inputs.clone(),
            disc: 0,
            state_bytes: 9,
        }),
        "mock".into(),
        sink.ring(),
        None,
        None,
    );
    let mask = ButtonMask(
        ButtonMask::X
            | ButtonMask::Y
            | ButtonMask::L
            | ButtonMask::R
            | ButtonMask::L2
            | ButtonMask::R2,
    );
    emu.set_input(mask);
    emu.set_driven(true);
    emu.set_speed(Speed::Normal);
    let until = Instant::now() + Duration::from_secs(2);
    while !emu.locked() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    let before = inputs.lock().unwrap().len();
    for _ in 0..60 {
        emu.tick(Duration::from_nanos(16_666_667));
        assert!(emu.wait_frame(Duration::from_millis(100)));
    }
    let log = inputs.lock().unwrap();
    assert!(
        (49..=51).contains(&(log.len() - before)),
        "PAL must run 50 frames per 60 display ticks"
    );
    assert!(
        log.iter().all(|&input| input == mask),
        "X/Y must stay independent buttons, including when turbo is enabled"
    );
}

fn psx_worker() -> slot::emu::EmuHandle {
    use slot::audio::{AudioSink, StubSink};
    let mut sink = StubSink::new();
    sink.open(44100).unwrap();
    slot::emu::EmuHandle::spawn(
        Box::new(DigitalCore {
            mock: slot_retro::MockCore::new(),
            inputs: Default::default(),
            disc: 0,
            state_bytes: 4 * 1024 * 1024,
        }),
        "mock".into(),
        sink.ring(),
        None,
        None,
    )
}

fn psx_state(emu: &slot::emu::EmuHandle) -> Vec<u8> {
    emu.request_state()
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
}

fn psx_frame(emu: &slot::emu::EmuHandle) -> u64 {
    u64::from_le_bytes(psx_state(emu)[..8].try_into().unwrap())
}

fn wait_psx(emu: &slot::emu::EmuHandle, condition: impl Fn(u64) -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !condition(psx_frame(emu)) {
        assert!(
            std::time::Instant::now() < until,
            "PS1 worker did not reach the expected frame"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn psx_rewinds_large_states_and_resumes_playback() {
    use slot::emu::Speed;
    let emu = psx_worker();
    emu.set_speed(Speed::Normal);
    wait_psx(&emu, |frame| frame >= 30);
    let played = psx_frame(&emu);
    emu.set_rewinding(true);
    wait_psx(&emu, |frame| frame < played);
    emu.set_speed(Speed::Paused);
    let rewound = psx_frame(&emu);
    assert!(
        rewound < played,
        "PS1 did not move backwards: {played} -> {rewound}"
    );
    emu.set_rewinding(false);
    emu.set_speed(Speed::Normal);
    wait_psx(&emu, |frame| frame > rewound);
}

#[test]
fn psx_rewind_cannot_cross_a_disc_swap_state_load_or_disabled_recording() {
    use slot::emu::Speed;
    use std::time::Duration;
    for boundary in 0..3 {
        let emu = psx_worker();
        emu.set_speed(Speed::Normal);
        wait_psx(&emu, |frame| frame >= 18);
        emu.set_speed(Speed::Paused);
        let mut state = psx_state(&emu);
        match boundary {
            0 => emu.change_disc(1),
            1 => {
                state[..8].copy_from_slice(&10000u64.to_le_bytes());
                emu.request_load(state);
            }
            _ => emu.set_rewind_recording(false),
        }
        std::thread::sleep(Duration::from_millis(60));
        let before = psx_state(&emu);
        emu.set_rewinding(true);
        emu.set_speed(Speed::Normal);
        std::thread::sleep(Duration::from_millis(180));
        emu.set_speed(Speed::Paused);
        assert_eq!(
            psx_state(&emu),
            before,
            "history survived boundary {boundary}"
        );
        if boundary == 0 {
            assert_eq!(emu.discs().0, 1);
        }
    }
}

#[test]
fn disc_menu_requests_valid_disc_changes_and_closes_without_ejecting() {
    let mut app = App::new(vec![Cart {
        platform: Platform::Psx,
        stem: "Adventure".into(),
        rom: "Adventure.m3u".into(),
        label: None,
        title: "Adventure".into(),
        code: String::new(),
        shell: None,
    }]);
    app.set_platform(Platform::Psx);
    app.apply(Action::Insert);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    assert!(
        app.taken_buttons().is_empty(),
        "all PlayStation shoulders belong to the game"
    );
    app.set_discs((0, 2, false));
    app.apply(Action::GameMenu);
    assert!(app.game_menu_open());
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(app.take_disc_change(), Some(1));
    app.set_discs((1, 2, false));
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        app.take_disc_change(),
        None,
        "next disc cannot exceed playlist"
    );
    app.apply(Action::GbaDown(Btn::Up));
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(app.take_disc_change(), Some(0));
    app.apply(Action::GbaDown(Btn::B));
    assert!(!app.game_menu_open());
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    assert!(app.may_rewind());
    assert!(app.may_fast_forward());
}
