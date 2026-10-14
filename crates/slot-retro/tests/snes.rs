#[path = "common/snes.rs"]
mod homebrew;

use slot_retro::{ButtonMask, LibretroCore, RetroCore};
use std::path::PathBuf;

#[test]
fn snes_homebrew_video_audio_two_ports_multitap_states_sram_and_pal() {
    let path = std::env::var_os("SLOT_TEST_SNES_CORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../target/desktop/snes9x_libretro.so"));
    if !path.exists() {
        assert!(
            std::env::var_os("SLOT_TEST_SNES_CORE").is_none(),
            "requested SNES core missing"
        );
        eprintln!("Snes9x unavailable; build it or set SLOT_TEST_SNES_CORE to run this test");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    for (pal, hires) in [(false, false), (true, true)] {
        let rom = root.path().join("input.sfc");
        std::fs::write(&rom, homebrew::rom(pal, hires)).unwrap();
        let mut core = LibretroCore::open(&path).unwrap();
        core.set_video_canvas(640, 480);
        core.load(&rom).unwrap();
        assert!(core.full_joypad());
        let fps = core.av_info().fps;
        assert!(if pal {
            (49.0..51.0).contains(&fps)
        } else {
            (59.0..61.0).contains(&fps)
        });
        assert!(core.av_info().sample_rate > 30_000.0);
        let pads = [
            ButtonMask(ButtonMask::X),
            ButtonMask(ButtonMask::Y),
            ButtonMask(ButtonMask::A),
            ButtonMask(ButtonMask::R),
        ];
        let hidden_picture = core.video_xrgb8888().to_vec();
        core.set_frame_skip(true);
        for _ in 0..10 {
            core.run_frame_pads(pads);
        }
        assert_eq!(
            core.video_xrgb8888(),
            hidden_picture,
            "hidden frames were rendered"
        );
        assert!(
            !core.take_audio().is_empty(),
            "hidden video stopped the audio clock"
        );
        core.set_frame_skip(false);
        for _ in 0..10 {
            core.run_frame_pads(pads);
        }
        let ram = core.save_ram().unwrap();
        assert_eq!(homebrew::buttons(&ram, 0), 0x40, "X must remain X");
        assert_eq!(
            homebrew::buttons(&ram, 1),
            0x4000,
            "port two must remain independent"
        );
        assert_eq!(core.video_xrgb8888().len(), 640 * 480 * 4);
        assert!(
            core.video_xrgb8888()
                .chunks_exact(4)
                .filter(|p| p[2] > 200 && p[0] < 10)
                .count()
                > 640 * 400
        );
        assert!(!core.take_audio().is_empty(), "no stereo audio frames");
        core.set_controller_port_device(1, 257);
        for _ in 0..10 {
            core.run_frame_pads(pads);
        }
        let ram = core.save_ram().unwrap();
        for (player, expected) in [0x40, 0x4000, 0x80, 0x10].into_iter().enumerate() {
            assert_eq!(
                homebrew::buttons(&ram, player),
                expected,
                "multitap player {}",
                player + 1
            );
        }
        let state = core.serialize().unwrap();
        for _ in 0..5 {
            core.run_frame_pads([ButtonMask(0); 4]);
        }
        core.unserialize(&state).unwrap();
        assert_eq!(core.save_ram().unwrap(), ram, "restore must recover SRAM");
        core.load_save_ram(&vec![0x5a; ram.len()]).unwrap();
        assert!(core.save_ram().unwrap().iter().all(|&byte| byte == 0x5a));
        let mut reusable = Vec::new();
        core.serialize_into(&mut reusable).unwrap();
        core.unserialize(&reusable).unwrap();
        core.set_controller_port_device(1, 1);
        for _ in 0..5 {
            core.run_frame_pads(pads);
        }
        assert_eq!(homebrew::buttons(&core.save_ram().unwrap(), 1), 0x4000);
    }
}
