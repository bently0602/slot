mod common;
#[path = "../../slot-retro/tests/common/snes.rs"]
#[allow(dead_code)]
mod snes_homebrew;

use std::path::{Path, PathBuf};

use slot_retro::{ButtonMask, LibretroCore, RetroCore};

fn core_path() -> Option<PathBuf> {
    std::env::var_os("SLOT_AUDIO_TEST_CORE")
        .map(PathBuf::from)
        .or_else(|| {
            let path = common::repo_root().join(format!(
                "target/desktop/mgba_libretro.{}",
                std::env::consts::DLL_EXTENSION
            ));
            path.is_file().then_some(path)
        })
        .or_else(common::vendored_core)
}

fn tone_rom(gba: bool) -> Vec<u8> {
    if gba {
        let mut rom = common::gba_rom();
        // ARM literal loads followed by halfword writes: turn on channel 2,
        // route it to both speakers, then leave a continuous square wave running.
        let registers = [
            (0x0400_0084u32, 0x0080u32),
            (0x0400_0080, 0xff77),
            (0x0400_0082, 0x0002),
            (0x0400_0068, 0xf080),
            (0x0400_006c, 0x87d6),
        ];
        let pool = 0xc0 + (registers.len() * 3 + 1) * 4;
        for (index, (address, value)) in registers.into_iter().enumerate() {
            let pc = 0xc0 + index * 12;
            let literal = pool + index * 8;
            let words = [
                0xe59f_0000 | (literal - pc - 8) as u32,
                0xe59f_1000 | (literal + 4 - pc - 12) as u32,
                0xe1c0_10b0,
            ];
            for (offset, word) in words.into_iter().enumerate() {
                rom[pc + offset * 4..pc + offset * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            rom[literal..literal + 4].copy_from_slice(&address.to_le_bytes());
            rom[literal + 4..literal + 8].copy_from_slice(&value.to_le_bytes());
        }
        let idle = pool - 4;
        rom[idle..idle + 4].copy_from_slice(&0xeaff_fffeu32.to_le_bytes());
        rom
    } else {
        let mut rom = vec![0; 0x8000];
        rom[0x100..0x104].copy_from_slice(&[0x00, 0xc3, 0x50, 0x01]);
        rom[0x104..0x108].copy_from_slice(&[0xce, 0xed, 0x66, 0x66]);
        rom[0x134..0x13e].copy_from_slice(b"SLOT AUDIO");
        rom[0x14d] = rom[0x134..0x14d]
            .iter()
            .fold(0u8, |a, b| a.wrapping_sub(*b).wrapping_sub(1));
        let registers = [
            (0x26u8, 0x80u8),
            (0x24, 0x77),
            (0x25, 0x22),
            (0x16, 0x80),
            (0x17, 0xf0),
            (0x18, 0xd6),
            (0x19, 0x87),
        ];
        let mut code = Vec::new();
        for (address, value) in registers {
            code.extend_from_slice(&[0x3e, value, 0xe0, address]);
        }
        code.extend_from_slice(&[0x18, 0xfe]);
        rom[0x150..0x150 + code.len()].copy_from_slice(&code);
        rom
    }
}

fn psx_tone_rom() -> Vec<u8> {
    // Original MIPS homebrew: upload one looping ADPCM block to SPU RAM and
    // leave voice 0 playing it. No commercial BIOS or game data is required.
    let mut exe = vec![0; 6144];
    exe[..8].copy_from_slice(b"PS-X EXE");
    for (offset, value) in [
        (0x10, 0x8001_0000u32),
        (0x18, 0x8001_0000),
        (0x1c, 4096),
        (0x30, 0x801f_fff0),
    ] {
        exe[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let mut code = vec![0x3c08_1f80u32]; // LUI t0, $1f80: MMIO base.
    for (offset, value) in [
        (0x1daau32, 0xc010u32), // SPU enabled, manual transfer.
        (0x1da6, 0x0200),       // Upload at SPU byte address $1000.
        (0x1da8, 0x0704),       // ADPCM shift 4, loop start/end/repeat.
        (0x1da8, 0x7777),
        (0x1da8, 0x7777),
        (0x1da8, 0x7777),
        (0x1da8, 0x8877),
        (0x1da8, 0x8888),
        (0x1da8, 0x8888),
        (0x1da8, 0x8888),
        (0x1d80, 0x3fff), // Main left/right volume.
        (0x1d82, 0x3fff),
        (0x1c00, 0x3fff), // Voice 0 left/right volume.
        (0x1c02, 0x3fff),
        (0x1c04, 0x0400), // Quarter-rate pitch, approximately 394 Hz.
        (0x1c06, 0x0200), // Voice start and repeat address.
        (0x1c0e, 0x0200),
        (0x1c08, 0x000f), // Fast attack and full sustain.
        (0x1c0a, 0x1fc0), // Hold sustain level.
        (0x1daa, 0xc000), // SPU enabled, unmuted.
        (0x1d88, 0x0001), // Key on voice 0.
    ] {
        code.extend([0x3409_0000 | value, 0xa509_0000 | offset]);
    }
    let pc = 0x8001_0000 + code.len() as u32 * 4;
    code.extend([0x0800_0000 | ((pc >> 2) & 0x03ff_ffff), 0]);
    for (i, word) in code.into_iter().enumerate() {
        exe[2048 + i * 4..2052 + i * 4].copy_from_slice(&word.to_le_bytes());
    }
    exe
}

fn capture(
    core_path: &Path,
    rom: &Path,
    ahead: u8,
    restore_only: bool,
    independent: bool,
) -> Vec<Vec<i16>> {
    let mut core = LibretroCore::open(core_path).expect("open real audio test core");
    if core.declared_options().contains_key("mgba_use_bios") {
        core.set_option("mgba_use_bios", "OFF");
        core.set_option("mgba_skip_bios", "ON");
    }
    if core.declared_options().contains_key("pcsx_rearmed_bios") {
        core.set_video_canvas(640, 480);
        core.set_option("pcsx_rearmed_bios", "HLE");
        core.set_option("pcsx_rearmed_memcard1", "libretro");
        core.set_option("pcsx_rearmed_memcard2", "none");
    }
    core.load(rom).expect("load generated tone ROM");
    for _ in 0..60 {
        core.run_frame(ButtonMask::default());
        core.take_audio();
    }
    let mut secondary = independent.then(|| core.run_ahead_instance().expect("independent core"));
    let mut frames = Vec::new();
    let mut state = Vec::new();
    for _ in 0..180 {
        core.set_frame_skip(ahead > 0);
        core.run_frame(ButtonMask::default());
        frames.push(core.take_audio());
        if ahead > 0 || restore_only {
            core.serialize_into(&mut state).expect("save real frame");
            let speculative: &mut dyn RetroCore = match secondary.as_mut() {
                Some(secondary) => {
                    secondary
                        .unserialize(&state)
                        .expect("sync speculative core");
                    secondary.as_mut()
                }
                None => &mut core,
            };
            for n in 0..ahead {
                speculative.set_frame_skip(n + 1 < ahead);
                speculative.run_frame(ButtonMask::default());
            }
            speculative.take_audio();
            if !independent {
                core.unserialize(&state).expect("restore real frame");
            }
        }
    }
    frames
}

fn check_audio(core_path: &Path, system: &str, rom_bytes: Vec<u8>) {
    let rom = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("run-ahead-tone.{system}"));
    std::fs::write(&rom, rom_bytes).expect("write generated tone ROM");
    let normal = capture(core_path, &rom, 0, false, false);
    assert!(
        normal.iter().flatten().any(|sample| *sample != 0),
        "tone ROM is silent"
    );
    let reference: Vec<_> = normal.iter().flatten().copied().collect();
    assert_eq!(
        normal,
        capture(core_path, &rom, 0, false, false),
        "baseline must be deterministic"
    );
    let mut mismatched = Vec::new();
    // Set only when reproducing the previous implementation. The assertions
    // deliberately fail if restoring the audible core changes its samples.
    let independent = std::env::var_os("SLOT_AUDIO_TEST_SINGLE_INSTANCE").is_none();
    for (ahead, restore_only) in [(0, true), (1, false), (2, false)] {
        let frames = capture(core_path, &rom, ahead, restore_only, independent);
        let samples: Vec<_> = frames.iter().flatten().copied().collect();
        let changed = reference
            .iter()
            .zip(&samples)
            .filter(|(a, b)| a != b)
            .count();
        let first_frame = normal.iter().zip(&frames).position(|(a, b)| a != b);
        eprintln!(
            "{system}: ahead {ahead}, restore only {restore_only}, samples {} vs {}, differing {changed}, first changed frame {first_frame:?}",
            samples.len(), reference.len()
        );
        if reference != samples {
            mismatched.push((ahead, restore_only, changed));
        }
    }
    assert!(
        mismatched.is_empty(),
        "{system} audio changed (independent core: {independent}): {mismatched:?}"
    );
}

#[test]
fn gb_run_ahead_preserves_the_normal_audio_stream() {
    let _guard = common::core_lock();
    let Some(core_path) = core_path() else {
        eprintln!("real-core audio test skipped: set SLOT_AUDIO_TEST_CORE to a host mGBA library");
        return;
    };
    check_audio(&core_path, "gb", tone_rom(false));
}

#[test]
fn gba_run_ahead_preserves_the_normal_audio_stream() {
    let _guard = common::core_lock();
    let Some(core_path) = core_path() else {
        eprintln!("real-core audio test skipped: set SLOT_AUDIO_TEST_CORE to a host mGBA library");
        return;
    };
    check_audio(&core_path, "gba", tone_rom(true));
}

#[test]
fn gpsp_run_ahead_preserves_the_normal_audio_stream() {
    let _guard = common::core_lock();
    let Some(core_path) = std::env::var_os("SLOT_GPSP_AUDIO_TEST_CORE") else {
        eprintln!("gpSP audio test skipped: set SLOT_GPSP_AUDIO_TEST_CORE to a host gpSP library");
        return;
    };
    check_audio(Path::new(&core_path), "gba", tone_rom(true));
}

#[test]
fn psx_run_ahead_preserves_the_normal_audio_stream() {
    let _guard = common::core_lock();
    let core_path = std::env::var_os("SLOT_TEST_PCSX_CORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            common::repo_root().join(format!(
                "target/desktop/pcsx_rearmed_libretro.{}",
                std::env::consts::DLL_EXTENSION
            ))
        });
    if !core_path.is_file() {
        assert!(
            std::env::var_os("SLOT_TEST_PCSX_CORE").is_none(),
            "requested PSX core missing"
        );
        eprintln!("PSX audio test skipped: set SLOT_TEST_PCSX_CORE to a host PCSX ReARMed library");
        return;
    }
    check_audio(&core_path, "exe", psx_tone_rom());
}

#[test]
fn snes_run_ahead_preserves_the_normal_audio_stream() {
    let _guard = common::core_lock();
    let core_path = std::env::var_os("SLOT_TEST_SNES_CORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            common::repo_root().join(format!(
                "target/desktop/snes9x_libretro.{}",
                std::env::consts::DLL_EXTENSION
            ))
        });
    if !core_path.is_file() {
        assert!(
            std::env::var_os("SLOT_TEST_SNES_CORE").is_none(),
            "requested SNES core missing"
        );
        eprintln!("SNES audio test skipped: set SLOT_TEST_SNES_CORE to a host Snes9x library");
        return;
    }
    check_audio(&core_path, "sfc", snes_homebrew::audio_rom());
}

#[test]
fn speculative_instance_does_not_modify_or_release_the_primary_core() {
    let _guard = common::core_lock();
    let Some(core_path) = core_path() else { return };
    let rom = Path::new(env!("CARGO_TARGET_TMPDIR")).join("independent-core.gba");
    std::fs::write(&rom, common::gba_rom()).unwrap();
    let mut primary = LibretroCore::open(&core_path).unwrap();
    primary.set_option("mgba_use_bios", "OFF");
    primary.load(&rom).unwrap();
    primary.run_frame(ButtonMask::default());
    let frame = primary.video_xrgb8888().to_vec();
    let audio = primary.take_audio();
    let state = primary.serialize().unwrap();
    let mut secondary = primary.run_ahead_instance().unwrap();
    secondary.unserialize(&state).unwrap();
    for _ in 0..2 {
        secondary.run_frame(ButtonMask::default());
    }
    assert_eq!(primary.video_xrgb8888(), frame);
    assert!(primary.take_audio().is_empty());
    assert!(!audio.is_empty());
    assert_ne!(secondary.video_xrgb8888(), frame);
    drop(secondary);
    assert!(
        LibretroCore::open(&core_path).is_err(),
        "dropping secondary released primary's ownership guard"
    );
    // Fail after the copied library has initialized, while loading its ROM.
    // Cleaning up that partial instance must also preserve the primary guard.
    std::fs::remove_file(&rom).unwrap();
    assert!(primary.run_ahead_instance().is_err());
    assert!(LibretroCore::open(&core_path).is_err());
    primary.run_frame(ButtonMask::default());
    assert!(!primary.take_audio().is_empty());
    drop(primary);
    assert!(LibretroCore::open(&core_path).is_ok());
}
