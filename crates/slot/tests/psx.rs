use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{Cart, Platform};

struct DigitalCore {
    mock: slot_retro::MockCore,
    inputs: std::sync::Arc<std::sync::Mutex<Vec<slot_retro::ButtonMask>>>,
}
impl slot_retro::RetroCore for DigitalCore {
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
        self.mock.serialize()
    }
    fn unserialize(&mut self, data: &[u8]) -> Result<(), slot_retro::CoreError> {
        self.mock.unserialize(data)
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
    assert!(!app.may_rewind());
    assert!(!app.may_fast_forward());
}
