use slot_store::{core_for_platform, scan, write_selected_core, Core, Platform, StateRing};

#[test]
fn snes_scans_its_shelf_and_always_selects_a_compatible_core() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("Games/SNES")).unwrap();
    std::fs::create_dir_all(root.path().join("Labels/SNES")).unwrap();
    for name in [
        "Game.sfc",
        "Game2.SMC",
        "Game3.swc",
        "Game4.fig",
        "skip.zip",
        "skip.gba",
    ] {
        std::fs::write(root.path().join("Games/SNES").join(name), []).unwrap();
    }
    std::fs::write(root.path().join("Labels/SNES/Game.png"), []).unwrap();
    let mut carts = scan(root.path()).unwrap();
    assert_eq!(carts.len(), 4);
    assert!(carts.iter().all(|c| c.platform == Platform::Snes));
    carts[0].read_header();
    assert_eq!(carts[0].title, "Game");
    assert!(carts[0]
        .label
        .as_ref()
        .unwrap()
        .ends_with("Labels/SNES/Game.png"));
    for core in Core::ALL {
        write_selected_core(root.path(), "Game", core).unwrap();
        assert_eq!(
            core_for_platform(root.path(), "Game", Platform::Snes),
            Core::Snes9x
        );
        assert_eq!(core.runs(Platform::Snes), core == Core::Snes9x);
    }
    for platform in Platform::ALL {
        assert_eq!(Core::Snes9x.runs(platform), platform == Platform::Snes);
    }
}

#[test]
fn snes_slots_history_and_resume_have_their_own_directory() {
    let root = tempfile::tempdir().unwrap();
    let ring = StateRing::new(root.path(), Platform::Snes, Core::Snes9x, "Game");
    for slot in 0..4 {
        ring.write_slot(slot, &[slot as u8]).unwrap();
        assert_eq!(ring.read_slot(slot).unwrap().unwrap(), [slot as u8]);
    }
    ring.write_resume(b"resume").unwrap();
    ring.push(b"history", &[], "2026-10-11_12-00-00").unwrap();
    assert_eq!(ring.list().unwrap().len(), 1);
    assert!(ring
        .slot_path(0)
        .unwrap()
        .ends_with("States/SNES/snes9x/Game/slot-1.state"));
    assert!(
        StateRing::new(root.path(), Platform::Gba, Core::Mgba, "Game")
            .read_slot(0)
            .unwrap()
            .is_none()
    );
}
