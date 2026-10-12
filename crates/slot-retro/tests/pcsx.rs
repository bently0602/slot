use slot_retro::{ButtonMask, LibretroCore, RetroCore};
use std::path::{Path, PathBuf};

// Entirely generated homebrew: a PS-X executable that paints red VRAM.
fn executable() -> Vec<u8> {
    let mut exe = vec![0; 6144];
    exe[..8].copy_from_slice(b"PS-X EXE");
    for (offset, value) in [
        (0x10, 0x80010000u32),
        (0x18, 0x80010000),
        (0x1c, 4096),
        (0x30, 0x801ffff0),
    ] {
        exe[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let mut code = vec![0x3c081f80u32, 0x35081810]; // t0 = GPU MMIO
    for (value, port) in [
        (0, 4),
        (0x03000000, 4),
        (0x08000001, 4),
        (0x06c60260, 4),
        (0x07042018, 4),
        (0xe1000000, 0),
        (0xe3000000, 0),
        (0xe403bd3f, 0),
        (0xe5000000, 0),
        (0x020000ff, 0),
        (0, 0),
        (0x00f00140, 0),
    ] {
        code.extend([
            0x3c090000 | (value >> 16),
            0x35290000 | (value & 0xffff),
            0xad090000 | port,
        ]);
    }
    let pc = 0x80010000 + code.len() as u32 * 4;
    code.extend([0x08000000 | ((pc >> 2) & 0x03ffffff), 0]);
    for (i, word) in code.iter().enumerate() {
        exe[2048 + i * 4..2052 + i * 4].copy_from_slice(&word.to_le_bytes());
    }
    exe
}

fn directory(name: &[u8], sector: u32, size: u32, is_dir: bool) -> Vec<u8> {
    let len = (33 + name.len() + 1) & !1;
    let mut rec = vec![0; len];
    rec[0] = len as u8;
    rec[2..6].copy_from_slice(&sector.to_le_bytes());
    rec[6..10].copy_from_slice(&sector.to_be_bytes());
    rec[10..14].copy_from_slice(&size.to_le_bytes());
    rec[14..18].copy_from_slice(&size.to_be_bytes());
    rec[25] = if is_dir { 2 } else { 0 };
    rec[28] = 1;
    rec[31] = 1;
    rec[32] = name.len() as u8;
    rec[33..33 + name.len()].copy_from_slice(name);
    rec
}

fn disc(root: &Path, stem: &str) {
    let exe = executable();
    let cnf = b"BOOT = cdrom:\\PSX.EXE;1\r\nTCB = 4\r\nEVENT = 10\r\nSTACK = 801ffff0\r\n";
    let mut iso = vec![0; 64 * 2048];
    let pvd = &mut iso[16 * 2048..17 * 2048];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 1;
    pvd[40..72].fill(b' ');
    pvd[40..49].copy_from_slice(b"SLOT_TEST");
    pvd[80..84].copy_from_slice(&64u32.to_le_bytes());
    pvd[84..88].copy_from_slice(&64u32.to_be_bytes());
    pvd[128..130].copy_from_slice(&2048u16.to_le_bytes());
    pvd[130..132].copy_from_slice(&2048u16.to_be_bytes());
    let root_rec = directory(&[0], 20, 2048, true);
    pvd[156..156 + root_rec.len()].copy_from_slice(&root_rec);
    let mut at = 20 * 2048;
    for rec in [
        root_rec,
        directory(&[1], 20, 2048, true),
        directory(b"SYSTEM.CNF;1", 21, cnf.len() as u32, false),
        directory(b"PSX.EXE;1", 22, exe.len() as u32, false),
    ] {
        iso[at..at + rec.len()].copy_from_slice(&rec);
        at += rec.len();
    }
    iso[21 * 2048..21 * 2048 + cnf.len()].copy_from_slice(cnf);
    iso[22 * 2048..22 * 2048 + exe.len()].copy_from_slice(&exe);
    std::fs::write(root.join(format!("{stem}.bin")), iso).unwrap();
    std::fs::write(
        root.join(format!("{stem}.cue")),
        format!("FILE \"{stem}.bin\" BINARY\n  TRACK 01 MODE1/2048\n    INDEX 01 00:00:00\n"),
    )
    .unwrap();
}

#[test]
fn homebrew_video_memory_card_and_multi_disc_resume() {
    let path = std::env::var_os("SLOT_TEST_PCSX_CORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/desktop/pcsx_rearmed_libretro.so")
        });
    if !path.exists() {
        eprintln!("build PCSX ReARMed to run this integration test");
        return;
    }
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pcsx-homebrew");
    std::fs::create_dir_all(&root).unwrap();
    disc(&root, "one");
    disc(&root, "two");
    let playlist = root.join("test.m3u");
    std::fs::write(&playlist, "# generated homebrew\none.cue\ntwo.cue\n").unwrap();
    let mut core = LibretroCore::open_with(&path, &root, &root).unwrap();
    core.set_video_canvas(640, 480);
    core.set_option("pcsx_rearmed_bios", "HLE");
    core.set_option("pcsx_rearmed_memcard1", "libretro");
    core.set_option("pcsx_rearmed_memcard2", "none");
    core.load(&playlist).unwrap();
    assert!(core.full_joypad());
    assert_eq!(core.discs(), (0, 2));
    for _ in 0..8 {
        core.run_frame(ButtonMask(0));
    }
    let frame = core.video_xrgb8888();
    assert_eq!(frame.len(), 640 * 480 * 4);
    assert!(
        frame
            .chunks_exact(4)
            .filter(|p| p[2] > 200 && p[0] < 10)
            .count()
            > 200_000,
        "homebrew full-screen red fill was cropped or not rendered"
    );
    let mut ram = core.save_ram().expect("memory card exposed to frontend");
    assert_eq!(ram.len(), 128 * 1024);
    ram[8192] = 123;
    core.load_save_ram(&ram).unwrap();
    assert_eq!(core.save_ram().unwrap()[8192], 123);
    core.change_disc(1).unwrap();
    assert_eq!(core.discs(), (1, 2));
    assert!(core.change_disc(2).is_err());
    assert_eq!(core.discs(), (1, 2));
    let state = core.serialize().unwrap();
    core.change_disc(0).unwrap();
    core.unserialize(&state).unwrap();
    assert_eq!(
        core.discs(),
        (1, 2),
        "resuming must restore the mounted disc"
    );
    // Rewind restores frequently, then steps with neutral pads to redraw.
    // Exercise the actual core, including its disc metadata, repeatedly.
    let began = std::time::Instant::now();
    let mut snapshots = Vec::new();
    for _ in 0..20 {
        for _ in 0..6 {
            core.run_frame_pads([ButtonMask(0); 4]);
            let _ = core.take_audio();
        }
        snapshots.push(core.serialize().unwrap());
    }
    eprintln!(
        "PCSX rewind: {} bytes per state; 120 frames + 20 captures in {:?}",
        snapshots[0].len(),
        began.elapsed()
    );
    for snapshot in snapshots.iter().rev() {
        core.unserialize(snapshot).unwrap();
        core.run_frame_pads([ButtonMask(0); 4]);
        let _ = core.take_audio();
        assert_eq!(core.discs(), (1, 2));
        assert!(core.video_xrgb8888().chunks_exact(4).any(|p| p[2] > 200));
    }
    drop(core);
    let mut core = LibretroCore::open_with(&path, &root, &root).unwrap();
    core.set_video_canvas(640, 480);
    core.set_option("pcsx_rearmed_bios", "HLE");
    let exe = root.join("test.exe");
    std::fs::write(&exe, executable()).unwrap();
    core.load(&exe).expect("standalone homebrew EXE loading");
}
