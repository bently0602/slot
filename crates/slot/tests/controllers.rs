use slot::input::controller::{button, AxisRange, Controller};
use slot::input::evdev::{Ev, EV_ABS, EV_KEY};
use slot::input::mux::InputMux;
use slot_input::{Btn, RawEvent};

fn key(code: u16, value: i32) -> Ev {
    Ev {
        kind: EV_KEY,
        code,
        value,
    }
}
fn axis(code: u16, value: i32) -> Ev {
    Ev {
        kind: EV_ABS,
        code,
        value,
    }
}

#[test]
fn external_buttons_keep_positions_and_do_not_use_the_gpio_layout() {
    for (code, btn) in [
        (0x130, Btn::B),
        (0x131, Btn::A),
        (0x133, Btn::X),
        (0x134, Btn::Y),
        (0x136, Btn::L1),
        (0x137, Btn::R1),
        (0x138, Btn::L2),
        (0x139, Btn::R2),
        (0x13a, Btn::Select),
        (0x13b, Btn::Start),
        (0x13c, Btn::Menu),
        (0x13d, Btn::Menu),
        (0x220, Btn::Up),
        (0x221, Btn::Down),
        (0x222, Btn::Left),
        (0x223, Btn::Right),
    ] {
        assert_eq!(button(code), Some(btn));
        let mut controller = Controller::default();
        assert_eq!(controller.feed(key(code, 1)), vec![RawEvent::Down(btn)]);
        assert!(controller.feed(key(code, 1)).is_empty());
        assert!(controller.feed(key(code, 2)).is_empty());
        assert_eq!(controller.feed(key(code, 0)), vec![RawEvent::Up(btn)]);
    }
    for code in [114, 115, 116, 0x162, 0x132] {
        assert_eq!(button(code), None);
    }
    assert_eq!(slot::input::evdev::code_to_btn(0x136), Some(Btn::Select));
}

#[test]
fn xpad_legacy_face_and_dpad_codes_keep_the_handheld_positions() {
    let mut controller = Controller::new(true);
    for (code, btn) in [
        (0x133, Btn::Y),
        (0x134, Btn::X),
        (0x2c0, Btn::Left),
        (0x2c1, Btn::Right),
        (0x2c2, Btn::Up),
        (0x2c3, Btn::Down),
    ] {
        assert_eq!(controller.feed(key(code, 1)), vec![RawEvent::Down(btn)]);
        assert_eq!(controller.feed(key(code, 0)), vec![RawEvent::Up(btn)]);
    }
}

#[test]
fn sticks_calibrate_unsigned_and_signed_ranges_with_a_dead_zone() {
    for range in [
        AxisRange {
            min: 0,
            max: 255,
            flat: 10,
        },
        AxisRange {
            min: -32768,
            max: 32767,
            flat: 128,
        },
    ] {
        let mut controller = Controller::default();
        controller.add_axis(0, range, None);
        let middle = ((i64::from(range.min) + i64::from(range.max)) / 2) as i32;
        assert!(controller.feed(axis(0, middle)).is_empty());
        assert_eq!(
            controller.feed(axis(0, range.min)),
            vec![RawEvent::Down(Btn::Left)]
        );
        assert!(controller.feed(axis(0, range.min + 1)).is_empty());
        assert_eq!(
            controller.feed(axis(0, range.max)),
            vec![RawEvent::Up(Btn::Left), RawEvent::Down(Btn::Right)]
        );
        assert_eq!(
            controller.feed(axis(0, middle + 1)),
            vec![RawEvent::Up(Btn::Right)]
        );
    }
}

#[test]
fn dpad_and_stick_or_digital_and_analog_triggers_share_a_hold_without_early_release() {
    let mut c = Controller::default();
    c.add_axis(
        0,
        AxisRange {
            min: -100,
            max: 100,
            flat: 0,
        },
        None,
    );
    c.add_axis(
        2,
        AxisRange {
            min: 0,
            max: 255,
            flat: 0,
        },
        Some(Btn::L2),
    );
    assert_eq!(c.feed(axis(0x10, -1)), vec![RawEvent::Down(Btn::Left)]);
    assert!(c.feed(axis(0, -100)).is_empty());
    assert!(c.feed(axis(0x10, 0)).is_empty());
    assert_eq!(c.feed(axis(0, 0)), vec![RawEvent::Up(Btn::Left)]);
    assert_eq!(c.feed(axis(2, 255)), vec![RawEvent::Down(Btn::L2)]);
    assert!(c.feed(key(0x138, 1)).is_empty());
    assert!(c.feed(axis(2, 0)).is_empty());
    assert_eq!(c.feed(key(0x138, 0)), vec![RawEvent::Up(Btn::L2)]);
}

#[test]
fn unplugging_one_pad_releases_its_buttons_and_preserves_the_handhelds_hold() {
    let mut mux = InputMux::default();
    assert_eq!(
        mux.feed(1, RawEvent::Down(Btn::A)),
        vec![RawEvent::Down(Btn::A)]
    );
    assert!(mux.feed(2, RawEvent::Down(Btn::A)).is_empty());
    assert_eq!(
        mux.feed(2, RawEvent::Down(Btn::L2)),
        vec![RawEvent::Down(Btn::L2)]
    );
    assert_eq!(mux.set(2, Vec::new()), vec![RawEvent::Up(Btn::L2)]);
    assert_eq!(
        mux.feed(1, RawEvent::Up(Btn::A)),
        vec![RawEvent::Up(Btn::A)]
    );
    assert!(mux.buttons().is_empty());
}

#[cfg(target_os = "linux")]
mod hotplug {
    use super::*;
    use slot_input::InputSource;
    use std::ffi::{c_char, CString};
    use std::fs::{self, File, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    unsafe extern "C" {
        fn mkfifo(path: *const c_char, mode: u32) -> i32;
    }

    fn fifo(path: &Path) {
        let name = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { mkfifo(name.as_ptr(), 0o600) }, 0);
    }

    fn writer(path: &Path) -> File {
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            if let Ok(file) = OpenOptions::new()
                .write(true)
                .custom_flags(0x800)
                .open(path)
            {
                return file;
            }
            assert!(Instant::now() < until, "controller was not discovered");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn send(file: &mut File, event: Ev) {
        let mut packet = [0u8; 24];
        packet[16..18].copy_from_slice(&event.kind.to_le_bytes());
        packet[18..20].copy_from_slice(&event.code.to_le_bytes());
        packet[20..24].copy_from_slice(&event.value.to_le_bytes());
        file.write_all(&packet).unwrap();
    }

    fn receive(input: &mut slot::input::DeviceInput, expected: &[RawEvent]) {
        let until = Instant::now() + Duration::from_secs(3);
        let mut events = Vec::new();
        while events.len() < expected.len() {
            events.extend(input.poll(0));
            assert!(
                Instant::now() < until,
                "wanted {expected:?}, got {events:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(events, expected);
    }

    #[test]
    fn a_usb_pad_connects_after_startup_disconnects_held_and_reuses_an_event_number() {
        let root = tempfile::tempdir().unwrap();
        let dev = root.path().join("dev");
        let sys = root.path().join("sys");
        fs::create_dir(&dev).unwrap();
        fs::create_dir(&sys).unwrap();
        let mut input = slot::input::DeviceInput::open_in(&dev, &sys, root.path(), false);
        let info = sys.join("event7/device");
        fs::create_dir_all(info.join("capabilities")).unwrap();
        fs::create_dir(info.join("id")).unwrap();
        fs::write(info.join("id/bustype"), "0003\n").unwrap();
        fs::write(info.join("name"), "8BitDo Ultimate\n").unwrap();
        std::os::unix::fs::symlink("/drivers/xpad", info.join("driver")).unwrap();
        fs::write(info.join("capabilities/key"), "1000000000000 0 0 0 0\n").unwrap();
        let path = dev.join("event7");
        fifo(&path);
        let mut pad = writer(&path);
        send(&mut pad, key(0x131, 1));
        send(&mut pad, key(0x133, 1));
        send(&mut pad, axis(0x10, -1));
        receive(
            &mut input,
            &[
                RawEvent::Down(Btn::A),
                RawEvent::Down(Btn::Y),
                RawEvent::Down(Btn::Left),
            ],
        );
        // A FIFO cannot answer the state ioctls, so recovery releases stale
        // holds. Events between SYN_DROPPED and SYN_REPORT must be ignored.
        send(
            &mut pad,
            Ev {
                kind: 0,
                code: 3,
                value: 0,
            },
        );
        send(&mut pad, key(0x131, 1));
        send(
            &mut pad,
            Ev {
                kind: 0,
                code: 0,
                value: 0,
            },
        );
        receive(
            &mut input,
            &[
                RawEvent::Up(Btn::A),
                RawEvent::Up(Btn::Y),
                RawEvent::Up(Btn::Left),
            ],
        );
        send(&mut pad, key(0x131, 1));
        send(&mut pad, key(0x133, 1));
        send(&mut pad, axis(0x10, -1));
        receive(
            &mut input,
            &[
                RawEvent::Down(Btn::A),
                RawEvent::Down(Btn::Y),
                RawEvent::Down(Btn::Left),
            ],
        );
        fs::remove_file(&path).unwrap();
        receive(
            &mut input,
            &[
                RawEvent::Up(Btn::A),
                RawEvent::Up(Btn::Y),
                RawEvent::Up(Btn::Left),
            ],
        );
        drop(pad);
        fifo(&path);
        let mut pad = writer(&path);
        send(&mut pad, key(0x131, 1));
        receive(&mut input, &[RawEvent::Down(Btn::A)]);
        let status = input.controller_status();
        let active = status
            .active
            .clone()
            .expect("reconnected controller missing from status");
        assert_eq!(status.connected[0].name, "8BitDo Ultimate");
        input.configure_controllers(&slot_input::ControllerConfig {
            profiles: vec![slot_input::ControllerProfile {
                id: active,
                swap_ab: true,
                ..slot_input::ControllerProfile::default()
            }],
            ..slot_input::ControllerConfig::default()
        });
        receive(&mut input, &[RawEvent::Up(Btn::A)]);
        send(&mut pad, key(0x131, 1));
        receive(&mut input, &[RawEvent::Down(Btn::B)]);
        drop(pad);
        receive(&mut input, &[RawEvent::Up(Btn::B)]);
    }
}
