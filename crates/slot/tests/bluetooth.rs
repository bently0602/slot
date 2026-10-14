mod common;

use slot::app::App;
use slot::audio::{playback_devices, AudioRoute};
use slot::bluetooth::Headphones;
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, AudioOutput, SlotState};
use slot_ui::QuickRow;

const MAC: [u8; 6] = [0xAA, 0xBB, 0xCC, 0x11, 0x22, 0x33];

#[test]
fn auto_prefers_a2dp_while_explicit_routes_keep_the_existing_outputs() {
    let cards = "0 [sndcodec]: sunxi-codec\n2 [sndhdmi]: HDMI";
    let pcms = "00-00: codec : playback 1\n02-00: HDMI : playback 1";
    let route = AudioRoute {
        output: AudioOutput::Auto,
        docked: true,
        bluetooth: Some(MAC),
    };
    assert_eq!(
        playback_devices(cards, pcms, route),
        ["bluealsa:DEV=AA:BB:CC:11:22:33,PROFILE=a2dp"]
    );
    assert_eq!(
        playback_devices(
            cards,
            pcms,
            AudioRoute {
                output: AudioOutput::Device,
                ..route
            }
        )[0],
        "plug:default"
    );
    assert_eq!(
        playback_devices(
            cards,
            pcms,
            AudioRoute {
                output: AudioOutput::Hdmi,
                ..route
            }
        )[0],
        "plughw:2,0"
    );
    assert_eq!(
        playback_devices(
            cards,
            pcms,
            AudioRoute {
                bluetooth: None,
                ..route
            }
        )[0],
        "plughw:2,0"
    );
}

#[test]
fn bluetooth_uses_headphone_volume_and_mute_even_with_hdmi_connected() {
    let root = common::tmp_root_with_carts(&[]);
    write_slot_state(
        root.path(),
        &SlotState {
            volume: 65,
            volume_hp: 25,
            muted_hp: true,
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut app = App::boot(root.path());
    app.set_hdmi_audio(true);
    assert_eq!(app.output_volume(), 65);
    app.set_bluetooth_audio(true);
    assert!(app.headphones_active());
    assert_eq!(app.volume(), 25);
    assert!(app.muted());
    assert_eq!(app.output_volume(), 0);
    app.set_bluetooth_audio(false);
    assert_eq!(app.output_volume(), 65);
}

#[test]
fn shelf_menu_opens_bluetooth_and_browsing_keeps_the_active_route() {
    let root = common::tmp_root_with_carts(&[]);
    common::clocked(root.path());
    let mut app = App::boot(root.path());
    let press = |app: &mut App, btn| {
        app.apply(Action::GbaDown(btn));
        app.apply(Action::GbaUp(btn));
    };
    app.apply(Action::QuickMenu);
    for _ in 0..QuickRow::Bluetooth.position() {
        press(&mut app, Btn::Down);
    }
    press(&mut app, Btn::A);
    assert_eq!(app.quick_menu(), Some(QuickRow::BluetoothScan));
    app.bluetooth.devices = vec![
        Headphones {
            address: MAC,
            name: "First".into(),
            paired: true,
            connected: true,
        },
        Headphones {
            address: [2; 6],
            name: "Second".into(),
            paired: false,
            connected: false,
        },
    ];
    app.bluetooth.active = Some(MAC);
    app.bluetooth.selected = Some(MAC);
    press(&mut app, Btn::Down);
    press(&mut app, Btn::Right);
    assert_eq!(app.bluetooth.text(), "Second");
    assert_eq!(app.bluetooth.active, Some(MAC));
    press(&mut app, Btn::B);
    assert_eq!(app.quick_menu(), Some(QuickRow::Bluetooth));
}

#[test]
fn turning_bluetooth_off_is_an_action_and_pauses_bluetooth_music() {
    let root = common::tmp_root_with_carts(&[]);
    common::clocked(root.path());
    let mut app = App::boot(root.path());
    app.set_bluetooth_audio(true);
    app.apply(Action::QuickMenu);
    for _ in 0..QuickRow::Bluetooth.position() {
        app.apply(Action::GbaDown(Btn::Down));
    }
    app.apply(Action::GbaDown(Btn::A));
    for _ in 0..QuickRow::BluetoothOff.position() {
        app.apply(Action::GbaDown(Btn::Down));
    }
    assert_eq!(app.quick_menu(), Some(QuickRow::BluetoothOff));
    assert!(app.quick_value(QuickRow::BluetoothOff).is_none());
    app.apply(Action::GbaDown(Btn::A));
    assert!(matches!(
        app.music.commands.as_slice(),
        [slot::music::Command::Pause]
    ));
    #[cfg(feature = "host")]
    assert_eq!(app.bluetooth.status, "Turn off in computer settings");
    app.apply(Action::GbaDown(Btn::B));
    assert_eq!(app.quick_menu(), Some(QuickRow::Bluetooth));
}
